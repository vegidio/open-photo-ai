package utils

import (
	"bytes"
	"encoding/binary"
	"image"
	"image/color"
	"image/jpeg"
	"math"
	"runtime"
	"sync"

	"github.com/cockroachdb/errors"
)

// Some DNGs store their main image in a compression the bundled LibRaw was not built to read. Lossy DNG (compression
// 34892: Adobe DNG Converter's and Lightroom's "lossy compressed" DNGs, and Smart Previews) needs LibRaw compiled with
// libjpeg, which raw-go's build is not, so LibRaw does not even recognise those files. JPEG XL DNG (compression 52546,
// DNG 1.7: DxO PureRAW's output, Lightroom's JPEG XL DNGs) needs LibRaw built with the Adobe DNG SDK, which it is not
// either. Such a file is rewritten here, in memory, with its tiles decompressed and every other tag left alone; LibRaw
// then reads it as the ordinary uncompressed DNG it has always supported, and keeps doing the linearisation, colour
// matrices and white balance itself.
//
// Lossy DNG samples are 8-bit and not linear: the file maps them to linear values with a per-channel polynomial in
// its OpcodeList2, which LibRaw applies only inside its lossy-DNG reader. So they are mapped here the same way, and
// written out as 16-bit linear samples with the white level LibRaw's lossy reader would have used.

const (
	tiffCompressionNone      = 1
	tiffCompressionLossyJPEG = 34892
	tiffCompressionJPEGXL    = 52546

	tagNewSubfileType   = 254
	tagImageWidth       = 256
	tagImageLength      = 257
	tagBitsPerSample    = 258
	tagCompression      = 259
	tagStripOffsets     = 273
	tagSamplesPerPixel  = 277
	tagRowsPerStrip     = 278
	tagStripByteCounts  = 279
	tagPlanarConfig     = 284
	tagTileWidth        = 322
	tagTileLength       = 323
	tagTileOffsets      = 324
	tagTileByteCounts   = 325
	tagSubIFDs          = 330
	tagWhiteLevel       = 50717
	tagOpcodeList2      = 51009
	tiffTypeUndefined   = 7
	tiffTypeShort       = 3
	tiffTypeLong        = 4
	tiffTypeIFD         = 13
	tiffEntrySize       = 12
	tiffHeaderSize      = 8
	tiffMaxIFDEntries   = 4096
	tiffMaxSubIFDsTried = 16

	dngOpcodeMapPolynomial = 8
	dngMaxPolynomialDegree = 8
)

// segmentCodec decodes one compressed tile or strip of a DNG main image.
type segmentCodec struct {
	name string
	// depth is the bits per sample the codec delivers; a file declaring anything else is rejected rather than having
	// its precision silently changed. 16-bit samples are written as they are; 8-bit ones are encoded values, mapped
	// to 16-bit linear through the file's curves (see lossyCurves).
	depth  uint32
	decode func(data []byte) (image.Image, error)
}

// outputDepth is the bits per sample of every rewritten image.
const outputDepth = 16

// dngCodecs holds the compressions expandCompressedDNG can undo, keyed by TIFF compression value.
var dngCodecs = map[uint32]segmentCodec{
	tiffCompressionLossyJPEG: {
		name:  "lossy JPEG",
		depth: 8,
		decode: func(data []byte) (image.Image, error) {
			return jpeg.Decode(bytes.NewReader(data))
		},
	},
	tiffCompressionJPEGXL: {
		name:   "JPEG XL",
		depth:  16,
		decode: func(data []byte) (image.Image, error) { return decodeJXL(data) },
	},
}

type tiffEntry struct {
	typ   uint16
	count uint32
	// pos is where the entry's value field sits in the file: the value itself when it fits in four bytes, otherwise
	// the offset of the out-of-line data.
	pos uint32
}

type tiffFile struct {
	data  []byte
	order binary.ByteOrder
}

// expandCompressedDNG returns data with its main image rewritten as uncompressed samples when that image uses one of
// dngCodecs, or data itself for any other file.
func expandCompressedDNG(data []byte) ([]byte, error) {
	f, ok := newTIFF(data)
	if !ok {
		return data, nil
	}

	entries, codec, found, err := f.findCompressedMainIFD()
	if err != nil || !found {
		return data, err
	}

	out, err := f.expand(entries, codec)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to expand %s DNG", codec.name)
	}

	return out, nil
}

func newTIFF(data []byte) (*tiffFile, bool) {
	if len(data) < tiffHeaderSize {
		return nil, false
	}

	var order binary.ByteOrder
	switch string(data[:2]) {
	case "II":
		order = binary.LittleEndian
	case "MM":
		order = binary.BigEndian
	default:
		return nil, false
	}

	if order.Uint16(data[2:]) != 42 {
		return nil, false
	}

	return &tiffFile{data: data, order: order}, true
}

// readIFD parses the directory at off into its entries, keyed by tag.
func (f *tiffFile) readIFD(off uint32) (map[uint16]tiffEntry, error) {
	if uint64(off)+2 > uint64(len(f.data)) {
		return nil, errors.New("TIFF directory offset out of range")
	}

	n := uint32(f.order.Uint16(f.data[off:]))
	if n > tiffMaxIFDEntries || uint64(off)+2+uint64(n)*tiffEntrySize > uint64(len(f.data)) {
		return nil, errors.New("TIFF directory out of range")
	}

	entries := make(map[uint16]tiffEntry, n)
	for i := range n {
		p := off + 2 + i*tiffEntrySize
		entries[f.order.Uint16(f.data[p:])] = tiffEntry{
			typ:   f.order.Uint16(f.data[p+2:]),
			count: f.order.Uint32(f.data[p+4:]),
			pos:   p + 8,
		}
	}

	return entries, nil
}

// valueOffset returns where e's values start, following the offset when they do not fit in the entry itself.
func (f *tiffFile) valueOffset(e tiffEntry, size uint32) (uint32, error) {
	total := uint64(e.count) * uint64(size)
	at := e.pos
	if total > 4 {
		at = f.order.Uint32(f.data[e.pos:])
	}

	if uint64(at)+total > uint64(len(f.data)) {
		return 0, errors.New("TIFF value out of range")
	}

	return at, nil
}

// uints reads a SHORT, LONG or IFD entry as unsigned integers.
func (f *tiffFile) uints(e tiffEntry) ([]uint32, error) {
	var size uint32
	switch e.typ {
	case tiffTypeShort:
		size = 2
	case tiffTypeLong, tiffTypeIFD:
		size = 4
	default:
		return nil, errors.Newf("unexpected TIFF field type %d", e.typ)
	}

	at, err := f.valueOffset(e, size)
	if err != nil {
		return nil, err
	}

	values := make([]uint32, e.count)
	for i := range values {
		p := at + uint32(i)*size
		if size == 2 {
			values[i] = uint32(f.order.Uint16(f.data[p:]))
		} else {
			values[i] = f.order.Uint32(f.data[p:])
		}
	}

	return values, nil
}

// uint reads the first value of tag, or def when the tag is absent.
func (f *tiffFile) uint(entries map[uint16]tiffEntry, tag uint16, def uint32) (uint32, error) {
	e, ok := entries[tag]
	if !ok {
		return def, nil
	}

	values, err := f.uints(e)
	if err != nil {
		return 0, err
	}
	if len(values) == 0 {
		return def, nil
	}

	return values[0], nil
}

// findCompressedMainIFD looks for the main image (NewSubfileType 0) in IFD0 and its SubIFDs, where DNG puts it, and
// reports it only when its compression is one of dngCodecs.
func (f *tiffFile) findCompressedMainIFD() (map[uint16]tiffEntry, segmentCodec, bool, error) {
	ifd0, err := f.readIFD(f.order.Uint32(f.data[4:]))
	if err != nil {
		return nil, segmentCodec{}, false, err
	}

	candidates := []map[uint16]tiffEntry{ifd0}
	if e, ok := ifd0[tagSubIFDs]; ok {
		offsets, err := f.uints(e)
		if err != nil {
			return nil, segmentCodec{}, false, err
		}

		for _, off := range offsets[:min(len(offsets), tiffMaxSubIFDsTried)] {
			sub, err := f.readIFD(off)
			if err != nil {
				return nil, segmentCodec{}, false, err
			}
			candidates = append(candidates, sub)
		}
	}

	for _, entries := range candidates {
		subfile, err := f.uint(entries, tagNewSubfileType, 0)
		if err != nil {
			return nil, segmentCodec{}, false, err
		}
		if subfile != 0 {
			continue
		}

		compression, err := f.uint(entries, tagCompression, tiffCompressionNone)
		if err != nil {
			return nil, segmentCodec{}, false, err
		}

		// The first main image decides: a DNG has one, and an uncompressed or LibRaw-native one is left alone.
		codec, ok := dngCodecs[compression]
		return entries, codec, ok, nil
	}

	return nil, segmentCodec{}, false, nil
}

// segmentLayout describes the tiles, or strips, of the main image.
type segmentLayout struct {
	offsets, counts        tiffEntry
	width, height          uint32 // of one segment as stored; strips shrink at the bottom, tiles are padded instead
	imageHeight, stripRows uint32
	tiled                  bool
	samplesPerPixel, depth uint32
}

// segmentBytes is the uncompressed size of segment i.
func (l segmentLayout) segmentBytes(i uint32) uint64 {
	w, h := l.segmentSize(i)
	return uint64(w) * uint64(h) * uint64(l.samplesPerPixel) * outputDepth / 8
}

func (f *tiffFile) layout(entries map[uint16]tiffEntry, codec segmentCodec) (segmentLayout, error) {
	var l segmentLayout
	var err error
	read := func(tag uint16, def uint32) uint32 {
		if err != nil {
			return 0
		}
		var v uint32
		v, err = f.uint(entries, tag, def)
		return v
	}

	width := read(tagImageWidth, 0)
	l.imageHeight = read(tagImageLength, 0)
	l.samplesPerPixel = read(tagSamplesPerPixel, 1)
	l.depth = read(tagBitsPerSample, 1)
	planar := read(tagPlanarConfig, 1)
	tileWidth := read(tagTileWidth, 0)
	tileLength := read(tagTileLength, 0)
	l.stripRows = read(tagRowsPerStrip, l.imageHeight)
	if err != nil {
		return l, err
	}

	switch {
	case width == 0 || l.imageHeight == 0:
		return l, errors.New("no image size")
	case l.depth != codec.depth:
		return l, errors.Newf("%d-bit samples are not supported, want %d-bit", l.depth, codec.depth)
	case l.samplesPerPixel != 1 && l.samplesPerPixel != 3:
		return l, errors.Newf("%d samples per pixel are not supported", l.samplesPerPixel)
	case planar != 1:
		return l, errors.New("planar images are not supported")
	}

	var ok1, ok2 bool
	if tileWidth > 0 && tileLength > 0 {
		l.tiled = true
		l.width, l.height = tileWidth, tileLength
		l.offsets, ok1 = entries[tagTileOffsets]
		l.counts, ok2 = entries[tagTileByteCounts]
	} else {
		if l.stripRows == 0 {
			l.stripRows = l.imageHeight
		}
		l.width, l.height = width, l.stripRows
		l.offsets, ok1 = entries[tagStripOffsets]
		l.counts, ok2 = entries[tagStripByteCounts]
	}

	switch {
	case !ok1 || !ok2 || l.offsets.count != l.counts.count || l.offsets.count == 0:
		return l, errors.New("malformed tile or strip tables")
	case l.offsets.typ != tiffTypeLong || l.counts.typ != tiffTypeLong:
		// The rewritten file is several times larger, so the tables must be able to hold 32-bit offsets.
		return l, errors.New("tile or strip tables stored as 16-bit values")
	}

	return l, nil
}

// segmentSize returns the width and height stored for segment i, which for strips is shorter at the bottom.
func (l segmentLayout) segmentSize(i uint32) (uint32, uint32) {
	if l.tiled {
		return l.width, l.height
	}

	rows := min(l.stripRows, l.imageHeight-min(l.imageHeight, i*l.stripRows))
	return l.width, rows
}

func (f *tiffFile) expand(entries map[uint16]tiffEntry, codec segmentCodec) ([]byte, error) {
	l, err := f.layout(entries, codec)
	if err != nil {
		return nil, err
	}

	offsets, err := f.uints(l.offsets)
	if err != nil {
		return nil, err
	}
	counts, err := f.uints(l.counts)
	if err != nil {
		return nil, err
	}

	var curves *[3][256]uint16
	if codec.depth == 8 {
		c, err := f.lossyCurves(entries)
		if err != nil {
			return nil, err
		}
		curves = &c
	}

	segments := make([][]byte, len(offsets))
	var total uint64
	for i := range offsets {
		if uint64(offsets[i])+uint64(counts[i]) > uint64(len(f.data)) {
			return nil, errors.New("tile or strip out of range")
		}

		total += l.segmentBytes(uint32(i)) + 1
	}

	// Checked before anything is decoded or allocated: the sizes come from the file, and the segment tables can only
	// hold 32-bit offsets.
	if uint64(len(f.data))+total > 1<<32-1 {
		return nil, errors.New("too large to expand")
	}

	// Segments are independent streams, so they decode in parallel; a 60 MP file has a few hundred tiles.
	var wg sync.WaitGroup
	var errMu sync.Mutex
	var firstErr error
	next := make(chan int)

	for range runtime.NumCPU() {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := range next {
				segment, err := f.decodeSegment(codec, f.data[offsets[i]:offsets[i]+counts[i]], l, uint32(i), curves)
				if err != nil {
					errMu.Lock()
					if firstErr == nil {
						firstErr = errors.Wrapf(err, "failed to decode segment %d", i)
					}
					errMu.Unlock()
					continue
				}
				segments[i] = segment
			}
		}()
	}

	for i := range offsets {
		next <- i
	}
	close(next)
	wg.Wait()

	if firstErr != nil {
		return nil, firstErr
	}

	// The decompressed segments are appended after the original bytes, which stay where they are so that every
	// other offset in the file is still valid; only the segment tables and the compression tag are patched.
	out := make([]byte, len(f.data), len(f.data)+int(total))
	copy(out, f.data)

	offsetsAt, err := f.valueOffset(l.offsets, 4)
	if err != nil {
		return nil, err
	}
	countsAt, err := f.valueOffset(l.counts, 4)
	if err != nil {
		return nil, err
	}

	for i, segment := range segments {
		if len(out)%2 == 1 {
			out = append(out, 0)
		}
		f.order.PutUint32(out[offsetsAt+uint32(i)*4:], uint32(len(out)))
		f.order.PutUint32(out[countsAt+uint32(i)*4:], uint32(len(segment)))
		out = append(out, segment...)
	}

	if err := f.patch(out, entries[tagCompression], tiffCompressionNone); err != nil {
		return nil, err
	}

	if curves != nil {
		// The samples are now 16-bit linear, with LibRaw's lossy-reader maximum as their white level.
		if err := f.patch(out, entries[tagBitsPerSample], outputDepth); err != nil {
			return nil, err
		}
		if e, ok := entries[tagWhiteLevel]; ok {
			if err := f.patch(out, e, math.MaxUint16); err != nil {
				return nil, err
			}
		}
	}

	return out, nil
}

// patch overwrites every value of the SHORT or LONG entry e in out with v.
func (f *tiffFile) patch(out []byte, e tiffEntry, v uint32) error {
	var size uint32
	switch e.typ {
	case tiffTypeShort:
		size = 2
	case tiffTypeLong:
		size = 4
	default:
		return errors.Newf("unexpected TIFF field type %d", e.typ)
	}

	at, err := f.valueOffset(e, size)
	if err != nil {
		return err
	}

	for i := range e.count {
		if size == 2 {
			f.order.PutUint16(out[at+i*2:], uint16(v))
		} else {
			f.order.PutUint32(out[at+i*4:], v)
		}
	}

	return nil
}

// lossyCurves builds the tables that map a lossy DNG's 8-bit samples to 16-bit linear ones, the way LibRaw's
// lossy_dng_load_raw does: from the MapPolynomial opcodes in OpcodeList2, one per plane, or an sRGB curve for a plane
// the file gives none for (LibRaw applies that curve when a file has no OpcodeList2 at all).
func (f *tiffFile) lossyCurves(entries map[uint16]tiffEntry) ([3][256]uint16, error) {
	var curves [3][256]uint16
	var set [3]bool

	if e, ok := entries[tagOpcodeList2]; ok && e.typ == tiffTypeUndefined {
		at, err := f.valueOffset(e, 1)
		if err != nil {
			return curves, err
		}

		// Opcode lists are big-endian whatever the file's byte order.
		list := f.data[at : at+e.count]
		be := binary.BigEndian
		if len(list) < 4 {
			return curves, errors.New("truncated OpcodeList2")
		}

		n, p := be.Uint32(list), 4
		for range n {
			if p+16 > len(list) {
				return curves, errors.New("truncated OpcodeList2")
			}
			opcode, size := be.Uint32(list[p:]), int(be.Uint32(list[p+12:]))
			p += 16
			if size < 0 || p+size > len(list) {
				return curves, errors.New("truncated OpcodeList2")
			}

			// MapPolynomial: area (4 x uint32), plane, planes, row pitch, column pitch, degree, then the
			// coefficients as doubles.
			if params := list[p : p+size]; opcode == dngOpcodeMapPolynomial && len(params) >= 36 {
				plane, planes, degree := be.Uint32(params[16:]), be.Uint32(params[20:]), be.Uint32(params[32:])
				if degree > dngMaxPolynomialDegree || len(params) < 36+8*int(degree+1) {
					return curves, errors.New("malformed MapPolynomial opcode")
				}

				var coeff [dngMaxPolynomialDegree + 1]float64
				for j := range degree + 1 {
					coeff[j] = math.Float64frombits(be.Uint64(params[36+8*j:]))
				}

				for c := plane; c < min(plane+max(planes, 1), 3); c++ {
					for i := range 256 {
						var total float64
						for j := range degree + 1 {
							total += coeff[j] * math.Pow(float64(i)/255, float64(j))
						}
						curves[c][i] = uint16(min(max(total*math.MaxUint16, 0), math.MaxUint16))
					}
					set[c] = true
				}
			}
			p += size
		}
	}

	srgb := srgbToLinearCurve()
	for c := range curves {
		if !set[c] {
			curves[c] = srgb
		}
	}

	return curves, nil
}

// srgbToLinearCurve ports LibRaw's gamma_curve(1/2.4, 12.92, 1, 255), the table its lossy DNG reader falls back to:
// the sRGB transfer function inverted, with the linear segment's breakpoint found the same way LibRaw finds it.
func srgbToLinearCurve() [256]uint16 {
	const power, slope = 1 / 2.4, 12.92
	var g2, g4 float64
	bnd := [2]float64{0, 1}
	for range 48 {
		g2 = (bnd[0] + bnd[1]) / 2
		if (math.Pow(g2/slope, -power)-1)/power-1/g2 > -1 {
			bnd[1] = g2
		} else {
			bnd[0] = g2
		}
	}
	g4 = g2 * (1/power - 1)

	var curve [256]uint16
	for i := range curve {
		r := float64(i) / 255
		if r >= 1 {
			curve[i] = math.MaxUint16
			continue
		}

		v := math.Pow((r+g4)/(1+g4), 1/power)
		if r < g2 {
			v = r / slope
		}
		curve[i] = uint16(0x10000 * v)
	}

	return curve
}

// decodeSegment decompresses segment i into its slot's pixels as interleaved 16-bit samples, in the file's byte order,
// mapping 8-bit samples through curves. A stream smaller than its slot is zero-padded, as the rows and columns past the
// image edge are never read.
func (f *tiffFile) decodeSegment(codec segmentCodec, data []byte, l segmentLayout, i uint32, curves *[3][256]uint16) (
	[]byte, error,
) {
	img, err := codec.decode(data)
	if err != nil {
		return nil, err
	}

	width, height := l.segmentSize(i)
	bounds := img.Bounds()
	if uint32(bounds.Dx()) > width || uint32(bounds.Dy()) > height {
		return nil, errors.Newf("segment is %dx%d, larger than its %dx%d slot", bounds.Dx(), bounds.Dy(), width, height)
	}

	spp := int(l.samplesPerPixel)
	out := make([]byte, l.segmentBytes(i))
	rowBytes := int(width) * spp * outputDepth / 8
	var px [3]uint16

	for y := range bounds.Dy() {
		row := out[y*rowBytes:]
		for x := range bounds.Dx() {
			if !pixelSamples(img, bounds.Min.X+x, bounds.Min.Y+y, l.depth, &px) {
				return nil, errors.Newf("segment decoded as %T, which does not carry %d-bit samples", img, l.depth)
			}

			// A single-sample image decodes as grey, so the first channel carries it.
			for c := range spp {
				v := px[c]
				if curves != nil {
					v = curves[c][v]
				}
				f.order.PutUint16(row[(x*spp+c)*2:], v)
			}
		}
	}

	return out, nil
}

// pixelSamples reads the red, green and blue samples of (x, y) at their stored precision, reporting false for an image
// type that does not hold depth-bit samples. The concrete types are read directly: going through color.Color would
// premultiply and rescale, and costs an allocation per pixel on images of tens of megapixels.
func pixelSamples(img image.Image, x, y int, depth uint32, px *[3]uint16) bool {
	switch m := img.(type) {
	case *image.YCbCr:
		if depth != 8 {
			return false
		}
		yi, ci := m.YOffset(x, y), m.COffset(x, y)
		r, g, b := color.YCbCrToRGB(m.Y[yi], m.Cb[ci], m.Cr[ci])
		px[0], px[1], px[2] = uint16(r), uint16(g), uint16(b)

	case *image.Gray:
		if depth != 8 {
			return false
		}
		v := uint16(m.Pix[m.PixOffset(x, y)])
		px[0], px[1], px[2] = v, v, v

	case *image.NRGBA:
		if depth != 8 {
			return false
		}
		p := m.Pix[m.PixOffset(x, y):]
		px[0], px[1], px[2] = uint16(p[0]), uint16(p[1]), uint16(p[2])

	case *image.NRGBA64:
		if depth != 16 {
			return false
		}
		p := m.Pix[m.PixOffset(x, y):]
		px[0] = binary.BigEndian.Uint16(p[0:])
		px[1] = binary.BigEndian.Uint16(p[2:])
		px[2] = binary.BigEndian.Uint16(p[4:])

	default:
		return false
	}

	return true
}
