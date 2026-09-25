import { Trans, useTranslation } from "react-i18next";
import logoTensorRT from "@/assets/logo_tensorrt.avif";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { track } from "@/lib/faro";
import { useSettingsStore } from "@/stores/settings";

/**
 * Screen 03: a machine that supports TensorRT is asked, once, whether to use it.
 *
 * Open for as long as it is mounted - `App` decides that - and **it cannot be dismissed**: answering is
 * the only way out, and answering is what takes it down.
 */
export const TensorRTDialog = () => {
    const { t } = useTranslation();
    const answerTensorRT = useSettingsStore((state) => state.answerTensorRT);

    // Whether people accept the slow first TensorRT build.
    const answer = (accepted: boolean) => {
        answerTensorRT(accepted);
        track("tensorrt_prompt_answered", { accepted });
    };

    return (
        <Dialog open>
            <DialogContent
                showCloseButton={false}
                aria-describedby={undefined}
                className="w-lg max-w-none gap-0 overflow-hidden rounded-xl border-border bg-card p-0 shadow-[0_24px_60px_rgb(0_0_0/0.7)] sm:max-w-none"
                onOpenAutoFocus={(event) => {
                    // As the setup dialog does, and for the same reason: Radix would focus No, and
                    // WebKit would draw that programmatic focus as a ring on a dialog where neither
                    // answer should look pre-chosen.
                    event.preventDefault();

                    const content = event.currentTarget;
                    if (content instanceof HTMLElement) content.focus();
                }}
                // An unanswered question is one the application would have to ask again on the next
                // launch, which is the repetition it exists to avoid - so neither Escape nor a click
                // outside closes it. The reference behaves the same way: it passes no `onClose`.
                onEscapeKeyDown={(event) => event.preventDefault()}
                onPointerDownOutside={(event) => event.preventDefault()}
                onInteractOutside={(event) => event.preventDefault()}
            >
                <DialogTitleBar title={t("dialogs.tensorRT.title")} showCloseButton={false} />

                <div className="flex flex-col items-center gap-6 p-6">
                    <img src={logoTensorRT} alt={t("dialogs.tensorRT.logoAlt")} className="h-24 w-auto" />

                    <div className="flex flex-col gap-3 text-center text-[13px] text-foreground leading-[1.6]">
                        <p className="text-pretty">{t("dialogs.tensorRT.intro")}</p>

                        {/*
                         * The emphasis is inside the sentence, so it travels with it: named tags
                         * rather than positional indices, so a translator can move it to wherever it
                         * belongs in their language.
                         */}
                        <p className="text-pretty">
                            <Trans
                                i18nKey="dialogs.tensorRT.optimization"
                                components={{ b: <span className="font-bold text-white" /> }}
                            />
                        </p>

                        <p>{t("dialogs.tensorRT.question")}</p>
                    </div>

                    <div className="flex gap-3">
                        <Button variant="secondary" className="min-w-36" onClick={() => answer(false)}>
                            {t("common.no")}
                        </Button>
                        <Button className="min-w-36" onClick={() => answer(true)}>
                            {t("common.yes")}
                        </Button>
                    </div>

                    <p className="text-pretty text-center text-foreground-dim text-xs leading-[1.6]">
                        <Trans
                            i18nKey="dialogs.tensorRT.footer"
                            components={{ b: <span className="font-bold" />, u: <span className="underline" /> }}
                        />
                    </p>
                </div>
            </DialogContent>
        </Dialog>
    );
};
