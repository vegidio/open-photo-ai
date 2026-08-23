import { useEffect, useState } from 'react';
import { Slider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';

type ValueSliderProps = TailwindProps & {
    value: number;
    onChange?: (value: number) => void;
    min?: number;
    max?: number;
    step?: number;
    marks?: { value: number; label: string }[];
};

/**
 * A slider with no companion text field — the value is read from the drag tooltip and the mark underneath it.
 *
 * Deliberately not IntensitySelector: that component's whole substance is the typeable TextField beside the slider,
 * and the string-typed `''`/`'-'` states a half-typed number needs. A number-valued control has none of that, and
 * merging the two would mean a `number | string` value contract plus a conditional field.
 */
export const ValueSlider = ({
    value,
    onChange,
    min = 0,
    max = 100,
    step = 1,
    marks,
    className = '',
}: ValueSliderProps) => {
    // Local, so dragging stays smooth even though onChange only fires once the user lets go.
    const [sliderValue, setSliderValue] = useState(value);

    // Keep in sync when the value changes from elsewhere (a different format selected, Settings' Cancel reverting).
    useEffect(() => {
        setSliderValue(value);
    }, [value]);

    return (
        <div className={`mx-1 ${className}`}>
            <Slider
                size='small'
                min={min}
                max={max}
                step={step}
                marks={marks}
                track={false}
                valueLabelDisplay='auto'
                value={sliderValue}
                onChange={(_, newValue) => setSliderValue(newValue)}
                // onChangeCommitted rather than onMouseUp: it also fires for keyboard adjustment, which the arrow keys
                // make available once the thumb has focus.
                onChangeCommitted={(_, newValue) => onChange?.(newValue)}
                // Lifts the value label over a sticky ListSubheader — in the Settings dialog the first row of a section
                // sits directly under one, and the label pops up into its band.
                //
                // The z-index alone does nothing here, which is worth explaining because it looks like it should be
                // enough: WebKit promotes `position: sticky` to its own compositing layer, and a composited layer
                // paints above non-composited content whatever the z-index says. `elementsFromPoint` disagrees with
                // what you see — it puts the label on top while the header still covers it — which is the tell.
                // Promoting the slider too puts both in the compositing tree, where the z-index is finally honoured.
                //
                // Scoped to the states that actually show the label: a resting slider has to keep scrolling *under*
                // that header like the rest of its row, and it must not hold a compositing layer for no reason.
                sx={{
                    '&:hover, &:focus-within, &:has(.MuiSlider-thumb.Mui-active)': {
                        zIndex: 2,
                        transform: 'translateZ(0)',
                    },
                }}
                className='block'
            />
        </div>
    );
};
