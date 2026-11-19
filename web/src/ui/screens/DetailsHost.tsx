import { useTranslation } from "react-i18next";
import { DetailsDialog } from "../attachments/DetailsDialog";
import type { useDetails } from "./useDetails";

/** The attachment details dialog of the main window (KINIG-136), open while `useDetails` has a state; the Rust error of a save is shown translated. */
export function DetailsHost({ details }: { details: ReturnType<typeof useDetails> }) {
  const { t } = useTranslation();
  const { state } = details;
  if (!state) return null;
  return (
    <DetailsDialog
      attachments={state.attachments}
      index={state.index}
      onIndexChange={details.setIndex}
      onSave={details.save}
      onClose={details.close}
      error={state.error ? t(`error.${state.error}`) : undefined}
    />
  );
}
