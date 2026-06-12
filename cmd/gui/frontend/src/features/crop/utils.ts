export const ROTATE_STEP = 90;
export const FULL_TURN = 360;

// Wrap an angle (in degrees) into the [0, 360) range.
export const normalizeAngle = (deg: number) => ((deg % FULL_TURN) + FULL_TURN) % FULL_TURN;

// Snap an angle down to the nearest ROTATE_STEP (90°) multiple.
export const snapToStep = (deg: number) => Math.floor(deg / ROTATE_STEP) * ROTATE_STEP;
