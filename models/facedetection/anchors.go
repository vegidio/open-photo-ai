package facedetection

// Prior represents an anchor box (also called a prior box) used in the RetinaFace face detection model.
//
// Anchor boxes are predefined bounding boxes at various positions, scales, and aspect ratios across the image that
// serve as reference points for detecting faces. The neural network predicts offsets relative to these anchors rather
// than absolute positions, making detection more efficient and accurate.
//
// Fields (all normalized to 0-1 range):
//   - cx, cy: center coordinates of the anchor box
//   - sx, sy: width and height (size/scale) of the anchor box
//
// During inference, the model:
//  1. Generates anchors at multiple scales (16×16, 32×32, 64×64, 256×256, 512×512) across different feature pyramid
//     levels with strides of 8, 16, and 32 pixels.
//  2. Predicts location offsets (loc), confidence scores (conf), and landmark positions relative to each anchor.
//  3. Decodes the predictions by applying the offsets to the anchor positions to produce final bounding boxes.
type Prior struct {
	cx, cy, sx, sy float32
}

// generateAnchors generates prior boxes (anchors) for RetinaFace
func generateAnchors(imageSize int) []Prior {
	// Configuration: minimum anchor sizes for each feature pyramid level
	minSizes := [][]int{{16, 32}, {64, 128}, {256, 512}}
	// Step sizes (stride) for each pyramid level
	steps := []int{8, 16, 32}

	totalAnchors := 0
	for k, step := range steps {
		featureH := (imageSize + step - 1) / step
		featureW := (imageSize + step - 1) / step
		totalAnchors += featureH * featureW * len(minSizes[k])
	}

	priors := make([]Prior, 0, totalAnchors)
	imageSizeFloat := float32(imageSize)

	for k, step := range steps {
		featureH := (imageSize + step - 1) / step
		featureW := (imageSize + step - 1) / step

		stepFloat := float32(step)
		stepNormalized := stepFloat / imageSizeFloat
		numMinSizes := len(minSizes[k])

		// Pre-calculate normalized anchor sizes
		normalizedSizes := make([]float32, numMinSizes)
		for idx, minSize := range minSizes[k] {
			normalizedSizes[idx] = float32(minSize) / imageSizeFloat
		}

		// Flatten the nested loops by iterating through all combinations
		totalFeatures := featureH * featureW
		for featureIdx := 0; featureIdx < totalFeatures; featureIdx++ {
			i := featureIdx / featureW
			j := featureIdx % featureW

			cy := (float32(i) + 0.5) * stepNormalized
			cx := (float32(j) + 0.5) * stepNormalized

			// Generate anchors for all sizes at this spatial location
			for _, sK := range normalizedSizes {
				priors = append(priors, Prior{
					cx: cx,
					cy: cy,
					sx: sK,
					sy: sK,
				})
			}
		}
	}

	return priors
}
