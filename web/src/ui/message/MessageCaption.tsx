/** The one-line caption of a single photo with a description and no text (KINIG-136): `text` is the description, or the muted "Description hidden" while the photo is covered. */
export interface Caption {
  text: string;
  muted: boolean;
}

/**
 * The description as the entry's text line: plain text cut with an ellipsis at the end of the line, with the same spacer as the last line of a text
 * (the time sits at the end of this line, as in a bubble with a text; there is no pill). Revealing swaps the text in this same line, so the
 * bubble does not change its height.
 */
export function MessageCaption({ caption }: { caption: Caption }) {
  return (
    <p className={caption.muted ? "message__caption message__caption--muted" : "message__caption"}>
      <span className="message__caption-text">{caption.text}</span>
      <span className="message__spacer" aria-hidden="true" />
    </p>
  );
}
