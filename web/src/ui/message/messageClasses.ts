/** What decides the look of a bubble. */
export interface BubbleLook {
  hasAttachments: boolean;
  /** KINIG-132: every attachment is a picture. */
  imagesOnly: boolean;
  /** Slice 5c: the entry is drawn as a sticker (no text, one uncovered sticker). */
  sticker: boolean;
  mode: "view" | "editing" | "deleted";
  failed: boolean;
  isTask: boolean;
  taskDone: boolean;
  /** The text is empty. */
  textless: boolean;
  /** The one line of the entry is a picture's description. */
  captioned: boolean;
  /** There are chips under the content. */
  reacted: boolean;
  hasQuote: boolean;
  highlighted: boolean;
  timeBelow: boolean;
  favorite: boolean;
  byAgent: boolean;
}

/**
 * The class list of a bubble and what Message needs to know about its frame.
 *
 * KINIG-132: every attachment is a picture and the bubble is a plain one: the album fills the bubble edge to edge, as in Telegram. Without text the
 * time moves onto the picture as a pill; without text and without a quote the bubble has no fill of its own.
 *
 * Slice 5c: a sticker is the same frameless bubble, always: the time is a pill on its corner, the chips go under it, and a quote above it has a fill
 * of its own (`message--sticker`). `stickerFrame` says the time pill lives inside the sticker's box.
 */
export function messageClasses(look: BubbleLook): { className: string; stickerFrame: boolean } {
  const { mode, failed, isTask, reacted } = look;
  const viewing = mode === "view";
  const stickerFrame = look.sticker && viewing && !failed && !isTask;
  const bleed = stickerFrame || (look.imagesOnly && look.hasAttachments && viewing && !failed && !isTask);
  const bleedPill = stickerFrame || (bleed && look.textless && !look.captioned && !reacted);
  const classes = ["message"];
  if (look.hasAttachments) classes.push("message--has-attachments");
  if (bleed) classes.push("message--bleed");
  if (bleedPill) classes.push("message--bleed-pill");
  if (stickerFrame) classes.push("message--bleed-bare", "message--sticker");
  else if (bleedPill && !look.hasQuote) classes.push("message--bleed-bare");
  if (look.highlighted) classes.push("message--highlighted");
  if (look.timeBelow && !stickerFrame) classes.push("message--time-below");
  if (reacted) classes.push("message--reacted");
  if (isTask) classes.push("message--task");
  if (isTask && look.taskDone) classes.push("message--task-done");
  if (look.favorite && mode !== "deleted") classes.push("message--favorite");
  if (look.byAgent && mode !== "deleted") classes.push("message--agent");
  if (mode === "editing") classes.push("message--editing");
  if (mode === "deleted") classes.push("message--deleted");
  return { className: classes.join(" "), stickerFrame };
}
