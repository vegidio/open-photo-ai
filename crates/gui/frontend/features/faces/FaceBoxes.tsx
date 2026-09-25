import { useTranslation } from "react-i18next";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { cn } from "@/lib/utils";

type FaceBoxesProps = {
    // The framed photograph's, because that is the space `detect_faces` answers in - `ipc/faces.ts`
    // says so. The dialog beneath already resolves them to draw the picture at that aspect, so they
    // are handed down rather than resolved a second time here.
    /** The framed photograph's width, which a box's horizontal shares are of. */
    width?: number;
    /** The framed photograph's height, which a box's vertical shares are of. */
    height?: number;
    /** The faces found, in the order the detector found them, which is what numbers them. */
    faces: Face[];
    /** The keys of the faces the user has skipped; every other face is chosen. */
    skipped: ReadonlySet<string>;
    /** Turns one face from chosen to skipped, or back. */
    onToggle: (face: Face) => void;
};

/** One coordinate as a share of the edge it is on, which is what a box is positioned in. */
const percent = (value: number, over: number) => `${(value / over) * 100}%`;

/**
 * One box over each face found, chosen ones yellow and skipped ones grey.
 *
 * **Positioned in percentages of the framed photograph, so nothing is measured.** The picture beneath
 * is drawn at exactly the framed photograph's aspect, so a share of its width lands on the same pixel
 * whatever size the element happens to be.
 *
 * **The scale is the framed photograph's own dimensions**, and nothing is drawn where they are not
 * known.
 *
 * **Each box is a button**, so every one of them is in the tab order and operable from the keyboard
 * without anything here arranging it, and each carries the number of the face it is over as its
 * accessible name.
 */
export const FaceBoxes = ({ width, height, faces, skipped, onToggle }: FaceBoxesProps) => {
    // Percentages remove the `displayWidth`/`displayHeight` props the reference threads down, along
    // with the measurement apparatus `SelectFacesDialog` describes. They are also what makes these
    // boxes testable at all: jsdom lays nothing out, so a box positioned from a measured element is a
    // box at `0,0,0,0` in every test, while a percentage is in the style attribute and can be asserted
    // against the coordinates it was computed from.
    const { t } = useTranslation();

    // Drawing nothing without a scale is the reference's own answer to a scale of zero.
    if (!width || !height) return;

    return (
        <div className="absolute inset-0" data-slot="face-boxes">
            {faces.map((face, index) => {
                const { min, max } = face.bounding_box;

                // The face's own key, which is what the selection is recorded under: two boxes that
                // shared one would be one face to everything else here, so they share a React key too.
                const key = faceKey(face);
                const chosen = !skipped.has(key);

                return (
                    <button
                        key={key}
                        type="button"
                        // Numbered by the detector's order, which is the order the run cache tag is
                        // folded from and the only thing that tells one box from another aloud.
                        aria-label={t("faces.toggleFace", { number: index + 1 })}
                        aria-pressed={chosen}
                        onClick={() => onToggle(face)}
                        className={cn(
                            "absolute box-border cursor-pointer rounded border-[3px] bg-transparent p-0",
                            chosen ? "border-warning" : "border-foreground-dim",
                        )}
                        style={{
                            left: percent(min.x, width),
                            top: percent(min.y, height),
                            width: percent(max.x - min.x, width),
                            height: percent(max.y - min.y, height),
                        }}
                    />
                );
            })}
        </div>
    );
};
