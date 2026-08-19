import { scaleOp } from './factory.ts';

export const Tokyo = scaleOp('up', 'tokyo');
export const Kyoto = scaleOp('up', 'kyoto');
export const Saitama = scaleOp('up', 'saitama');

// Osaka's wire ID carries the scale like every other upscale model, but the Go operation drops it from the
// model identity: SeedVR2 restores at whatever size it is given, so one set of sessions serves every scale.
export const Osaka = scaleOp('up', 'osaka');
