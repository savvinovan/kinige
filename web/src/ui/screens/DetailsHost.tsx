import { useMemo, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { DetailsDialog } from "../attachments/DetailsDialog";
import type { useDetails } from "./useDetails";

/**
 * The attachment details dialog of the main window (KINIG-136), open while `useDetails` has a state; the Rust error of a save is shown translated.
 *
 * On close the focus goes back to the button the dialog was opened on, found in the page at that moment: a tile's "Details", or the photo or file
 * card. The Modal's own "the element that was focused when it opened" is wrong here: in WebKit a click does not focus a button, and a menu item that
 * was focused has gone by then, so the focus would return to whatever was focused before (a photo far up the feed, which scrolls the feed to it).
 */
export function DetailsHost({ details }: { details: ReturnType<typeof useDetails> }) {
  const { t } = useTranslation();
  const { state } = details;
  const scope = state?.scope;
  const returnTo = state?.returnTo;
  const returnFocusRef = useMemo<RefObject<HTMLElement | null>>(
    () => ({
      get current() {
        if (returnTo === undefined) return null;
        const draft = scope === "draft";
        const candidates = document.querySelectorAll<HTMLElement>(draft ? "[data-details-for]" : "[data-attachment-id]");
        return [...candidates].find((element) => (draft ? element.dataset.detailsFor : element.dataset.attachmentId) === returnTo) ?? null;
      },
      set current(_value) {
        // Read only: the element is looked up when the dialog closes.
      },
    }),
    [scope, returnTo],
  );
  if (!state) return null;
  return (
    <DetailsDialog
      attachments={state.attachments}
      index={state.index}
      onIndexChange={details.setIndex}
      onSave={details.save}
      onClose={details.close}
      returnFocusRef={returnFocusRef}
      error={state.error ? t(`error.${state.error}`) : undefined}
    />
  );
}
