package upscale

import (
	"encoding/binary"
	"fmt"

	"github.com/vegidio/open-photo-ai/types"
	"github.com/x448/float16"
	ort "github.com/yalue/onnxruntime_go"
)

func createTensor(data []float32, h, w int, precision types.Precision) ([]ort.Value, error) {
	switch precision {
	case types.PrecisionFp32:
		return createTensorFp32(data, h, w)
	case types.PrecisionFp16:
		return createTensorFp16(data, h, w)
	default:
		return nil, fmt.Errorf("unsupported precision: %v", precision)
	}
}

func createEmptyTensor(h, w int, precision types.Precision) ([]ort.Value, error) {
	switch precision {
	case types.PrecisionFp32:
		return createEmptyTensorFp32(h, w)
	case types.PrecisionFp16:
		return createEmptyTensorFp16(h, w)
	default:
		return nil, fmt.Errorf("unsupported precision: %v", precision)
	}
}

// region - FP32 functions

func createTensorFp32(data []float32, h, w int) ([]ort.Value, error) {
	shape := ort.NewShape(1, 3, int64(h), int64(w))
	tensor, err := ort.NewTensor[float32](shape, data)
	if err != nil {
		return nil, err
	}

	return []ort.Value{tensor}, nil
}

func createEmptyTensorFp32(h, w int) ([]ort.Value, error) {
	shape := ort.NewShape(1, 3, int64(h), int64(w))
	tensor, err := ort.NewEmptyTensor[float32](shape)
	if err != nil {
		return nil, err
	}

	return []ort.Value{tensor}, nil
}

// endregion

// region - FP16 functions

func createTensorFp16(data []float32, h, w int) ([]ort.Value, error) {
	customData := make([]byte, len(data)*2)
	for i, v := range data {
		f16 := float16.Fromfloat32(v) // Convert to float16
		binary.LittleEndian.PutUint16(customData[i*2:], f16.Bits())
	}

	shape := ort.NewShape(1, 3, int64(h), int64(w))
	tensor, err := ort.NewCustomDataTensor(shape, customData, ort.TensorElementDataTypeFloat16)
	if err != nil {
		return nil, err
	}

	return []ort.Value{tensor}, nil
}

func createEmptyTensorFp16(h, w int) ([]ort.Value, error) {
	shape := ort.NewShape(1, 3, int64(h), int64(w))
	// For FP16, calculate the total number of bytes needed
	totalElements := 1 * 3 * h * w
	customData := make([]byte, totalElements*2) // 2 bytes per float16

	tensor, err := ort.NewCustomDataTensor(shape, customData, ort.TensorElementDataTypeFloat16)
	if err != nil {
		return nil, err
	}

	return []ort.Value{tensor}, nil
}

// endregion

func valueToTensorData(value []ort.Value, valuePrecision types.Precision) ([]float32, ort.Shape, error) {
	switch valuePrecision {
	case types.PrecisionFp32:
		t := value[0].(*ort.Tensor[float32])
		return t.GetData(), t.GetShape(), nil
	case types.PrecisionFp16:
		t := value[0].(*ort.CustomDataTensor)

		data32 := make([]float32, len(t.GetData())/2)
		for i := range data32 {
			valueUint16 := binary.LittleEndian.Uint16(t.GetData()[i*2:])
			valueFloat16 := float16.Frombits(valueUint16)
			data32[i] = valueFloat16.Float32()
		}

		return data32, t.GetShape(), nil
	default:
		return nil, nil, fmt.Errorf("unsupported precision: %v", valuePrecision)
	}
}
