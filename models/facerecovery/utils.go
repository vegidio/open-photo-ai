package facerecovery

import (
	"image"
)

// addPadding expands a rectangle by a specified padding percentage on all sides.
//
// This function is used to create a larger area around detected face edges, giving the model more contextual
// information about the surrounding pixels. This additional context helps the model produce more natural and accurate
// results when restoring facial features.
//
// # Parameters:
//   - rect: The original rectangle to expand
//   - imgBounds: The bounds of the image to clamp the result within
//   - padding: The padding percentage (e.g., 0.2 for 20% expansion)
//
// Returns an expanded rectangle clamped to stay within the image bounds.
func addPadding(rect image.Rectangle, imgBounds image.Rectangle, padding float64) image.Rectangle {
	width := rect.Dx()
	height := rect.Dy()

	padX := int(float64(width) * padding)
	padY := int(float64(height) * padding)

	newRect := image.Rect(
		rect.Min.X-padX,
		rect.Min.Y-padY,
		rect.Max.X+padX,
		rect.Max.Y+padY,
	)

	// Clamp to image bounds
	if newRect.Min.X < imgBounds.Min.X {
		newRect.Min.X = imgBounds.Min.X
	}
	if newRect.Min.Y < imgBounds.Min.Y {
		newRect.Min.Y = imgBounds.Min.Y
	}
	if newRect.Max.X > imgBounds.Max.X {
		newRect.Max.X = imgBounds.Max.X
	}
	if newRect.Max.Y > imgBounds.Max.Y {
		newRect.Max.Y = imgBounds.Max.Y
	}

	return newRect
}

// makeSquare converts a rectangle into a square by using the larger dimension as the square size.
//
// This function is used to transform detected face regions into square shapes, which is required by the model's input
// format. The square is centered on the original rectangle to maintain the face's position while ensuring all
// dimensions meet the model's requirements.
//
// # Parameters:
//   - rect: The original rectangle to convert into a square
//   - imgBounds: The bounds of the image to clamp the result within
//
// Return a square rectangle centered on the original, clamped to stay within the image bounds.
func makeSquare(rect image.Rectangle, imgBounds image.Rectangle) image.Rectangle {
	width := rect.Dx()
	height := rect.Dy()

	// Use the larger dimension as the square size
	size := width
	if height > width {
		size = height
	}

	// Calculate the center of the rectangle
	centerX := rect.Min.X + width/2
	centerY := rect.Min.Y + height/2

	// Create a square centered on the original rectangle
	halfSize := size / 2
	minX := centerX - halfSize
	maxX := minX + size
	minY := centerY - halfSize
	maxY := minY + size

	// Clamp both axes using the helper function
	minX, maxX = clampAxis(minX, maxX, imgBounds.Min.X, imgBounds.Max.X)
	minY, maxY = clampAxis(minY, maxY, imgBounds.Min.Y, imgBounds.Max.Y)

	return image.Rect(minX, minY, maxX, maxY)
}

// clampAxis clamps a 1D range [min, max] to stay within [boundsMin, boundsMax] while trying to maintain the original
// size by shifting when possible.
func clampAxis(min, max, boundsMin, boundsMax int) (int, int) {
	if min < boundsMin {
		diff := boundsMin - min
		min = boundsMin
		max += diff
		if max > boundsMax {
			max = boundsMax
		}
	} else if max > boundsMax {
		diff := max - boundsMax
		max = boundsMax
		min -= diff
		if min < boundsMin {
			min = boundsMin
		}
	}
	return min, max
}
