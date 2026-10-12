## Slices

Not a slice of a series. It closes a gap left by `add-gui-enhance-1-seam`, which is archived.

| Slice | Delivers | Depends on | State |
| --- | --- | --- | --- |
| `add-gui-enhance-1-seam` | The structured operation on the wire, resolved against the catalogue | - | Done |
| `add-gui-enhance-2-sidebar` | The screen that sends one | Slice 1 | Done |
| `add-catalogue-build` | The catalogue building what it describes, so the GUI names no model | Slice 1 | **This change** |

## How the work divides

- **`opai`** gains the construction seam beside the catalogue it already publishes, and the refusal
  type that names what was wrong. This is the whole of the new logic.
- **`gui`** loses code. `operation.rs` drops its codename match and both precision mappers, and
  `Requested::resolve` becomes the scale clamping it owns plus one call.
- **`cli`** is untouched by this change but is the reason the seam belongs in `opai`: it needs the
  same conversion and would otherwise write a second copy of the match.

## D1 - The seam is on the catalogue, not a free function over strings

The construction belongs on `VariantEntry`, or on a lookup that returns one, rather than a
`from_codename(&str)` taking bare strings.

The reason is that the entry is what *knows*. It already carries the precisions this variant is
published at and the parameters it takes; a free function over strings would have to look the entry up
again to check them, which is the duplication this change exists to remove. Building from the entry
means the check and the construction read the same field.

It also makes the failure modes exhaustive by construction. There are exactly two ways a request can
be wrong - a codename no entry has, and a precision this entry does not list - and both are decided by
data the entry holds.

## D2 - Parameters are passed, not stored

An `Operation` carries values (an upscale's scale, face recovery's fidelity); a `VariantEntry` carries
their *bounds*. The build therefore takes the values as an argument and the entry supplies everything
else.

Bounds stay where they are. `ParameterEntry` already publishes them, the GUI already builds its
controls from them, and `Scale::clamped` already exists for the clamping. This change does not move
clamping into the library: `operation.rs` keeps it, because the decision to clamp rather than refuse
(slice 1's design.md D7) is about what a bounded slider's out-of-range value means, which is a
statement about a control.

## D3 - The precision mappers disappear rather than move

`float()` and `osaka()` exist because the model constructors take different precision enums -
`FloatPrecision` for the three convolutional upscalers, `OsakaPrecision` for the diffusion one - and
something has to map the wire's `Precision` onto the right one.

Inside `opai` there is nothing to map. The variant's own constructor is reached through its own arm, so
the enum is known statically at the one place that calls it. The mapping was only ever needed because
the caller was outside the library and had to guess which enum a codename implied - which is exactly
the coupling being removed.

`osaka()`'s "no FP32 build" special case goes the same way. It is already stated as data:
`OsakaPrecision::PRECISIONS` is `[Fp16, Int8]`, and the entry publishes it. The check becomes
`precisions.contains(&precision)` for every variant, with no arm for the one that differs.

## D4 - The existing family seams are unchanged

The project config fixes how a family exposes its models: one match from the variant enum to
`&'static dyn <Family>Model`, and one match from the resolved contract to a `Pipeline`. This change
adds neither. It adds a third, narrower mapping - published codename to that family's variant enum -
which is a lookup the catalogue is already built from.

That matters for the "a new model is a directory plus one arm" property. After this change it stays a
directory plus one arm; what goes away is the *second* arm, in another crate, that nothing made
mandatory.

## Ownership and lifetime

Nothing here touches sessions, pools, budgets or execution providers. The catalogue is
`&'static [FamilyEntry]` behind a `LazyLock`, built once; `VariantEntry` holds `&'static` borrows
throughout. Building an `Operation` allocates the operation and reads static data, and a refusal
allocates its message. No lease is taken and no session can be reached from any of it.

The model registry's ownership model is therefore unchanged, and deliberately so - this change should
not be able to affect it.

## Platforms

Platform-independent: it is a data lookup and a constructor call, with no IO and no provider
involvement. Validated on macOS (arm64). Windows and Linux carry no distinct code path here.
