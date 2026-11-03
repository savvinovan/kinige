import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { Icon } from "../icons/Icon";
import { positionBelow, type MenuPosition } from "../menu/ContextMenu";
import { renderMarkdown } from "./markdown";
import { MessageActions } from "./MessageActions";
import { MessageEditor } from "./MessageEditor";
import { MessageMenu } from "./MessageMenu";
import type { MoveTarget } from "./moveTargets";
import { selectedTextIn, useCopy } from "./useCopy";
import { useActionsPlacement } from "./useActionsPlacement";
import "./message.css";
import "./markdown.css";

export interface MessageProps {
  /** Message text in Markdown. Raw HTML in it is not rendered (§6). */
  markdown: string;
  /** When it was sent; the time is shown in 24-hour format in the entry's time zone (`timeZone`), or in the window's zone without it. */
  sentAt: Date;
  /** IANA time zone the entry was created in (`created_tz`). */
  timeZone?: string;
  /** "failed" - not sent: a "Not sent. Retry" line under the text. */
  status?: "sent" | "failed";
  /** Highlighted message (found by search, jump by link): a different fill of the bubble. */
  highlighted?: boolean;
  onRetry?: () => void;
  /** Click on a link in the text: the address goes to the system browser (the `open_external_url` command). Without a handler the link opens nothing. */
  onOpenLink?: (url: string) => void;
  /** Show the action row without hover (for stories and "hover" state snapshots). */
  showActions?: boolean;

  // Slice 3. All props are optional; without handlers the D1 behavior: buttons are drawn and disabled.
  /** The entry is a task: a checkbox before the text. */
  isTask?: boolean;
  /** The task is done: the checkbox is checked, the text is muted. */
  taskDone?: boolean;
  /** Favorite: a quiet star next to the time. */
  favorite?: boolean;
  /** The entry was added by an AI agent (`origin = 'agent'`, slice 4a): a quiet "added by agent" mark in the time line. */
  byAgent?: boolean;
  /** The entry was created by a newer version (group in an unsupported `sv`): actions are disabled, copying is allowed. */
  readOnly?: boolean;
  /** Reply quote (slice 5, `ReplyQuote`): sits at the top of the bubble, above the text. Normal mode only. */
  quote?: ReactNode;
  /** Entry tags (lower case, without `#`), determined by Rust: highlighted and clickable in the text. */
  tags?: readonly string[];
  /** Words of the entry found by search, as written in it (Rust returns them): highlighted whole (slice 4). */
  markTerms?: readonly string[];
  /**
   * view - normal; editing - the text is replaced by the edit field; deleted - muted, "Deleted · Restore" instead of the time,
   * no action row and no menu (decision 1 of slice 3).
   */
  mode?: "view" | "editing" | "deleted";
  /** Start editing (row button, menu item). The parent turns `editing` mode on. */
  onEdit?: () => void;
  /** Enter in the edit field: non-empty text. */
  onSaveEdit?: (text: string) => void;
  /** Esc in the edit field. */
  onCancelEdit?: () => void;
  /** Already translated save-edit error: a `role="alert"` line inside the bubble. */
  editError?: string;
  /** Item "Make a task" / "Remove task": `on` is the new state. */
  onToggleTask?: (on: boolean) => void;
  /** Click on the task checkbox: `done` is the new state. */
  onToggleDone?: (done: boolean) => void;
  onToggleFavorite?: (on: boolean) => void;
  onChangeDate?: () => void;
  onDelete?: () => void;
  onRestore?: () => void;
  /** Click on `- [ ]` inside the text: `index` is the box's ordinal number in the bubble (from zero), `checked` is the new state. */
  onToggleTaskItem?: (index: number, checked: boolean) => void;
  /** Click on a tag: the tag in lower case, without `#`. */
  onTagClick?: (tag: string) => void;
  /** Acceptance decision: chats for the "Move to chat" item (the current one is marked) and the move to the chosen one. Without a handler the item is disabled. */
  moveTargets?: readonly MoveTarget[];
  onMoveTo?: (chatId: string) => void;
  /**
   * Mouse button press on the bubble: the parent decides whether to start dragging to a chat (`useRecordDrag`). The bubble hands over
   * the event and the element itself (its copy is the "ghost"). Not called while editing, on a deleted bubble and on a read-only entry.
   */
  onDragStart?: (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
}

/**
 * A message is a bubble without a tail and avatar; the time is small in the bottom right corner on the same line as the end of the text.
 * The gap to neighboring bubbles is set by the feed (a series - smaller), here is only the bubble itself.
 *
 * Slice 3: the action row and the context menu (right click, "more", Shift+F10 or the menu key on any element of the bubble),
 * editing in the bubble, the task checkbox, the "favorite" mark, the deleted bubble, clickable tags and `- [ ]` boxes.
 * The component is "dumb": state (mode, flags) and handlers come from outside.
 */
export function Message({
  markdown,
  sentAt,
  timeZone,
  status = "sent",
  highlighted = false,
  onRetry,
  onOpenLink,
  showActions = false,
  isTask = false,
  taskDone = false,
  favorite = false,
  byAgent = false,
  readOnly = false,
  quote,
  tags,
  markTerms,
  mode = "view",
  onEdit,
  onSaveEdit,
  onCancelEdit,
  editError,
  onToggleTask,
  onToggleDone,
  onToggleFavorite,
  onChangeDate,
  onDelete,
  onRestore,
  onToggleTaskItem,
  onTagClick,
  moveTargets,
  onMoveTo,
  onDragStart,
}: MessageProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const failed = status === "failed";
  const viewing = mode === "view";
  const labelDone = t("message.taskDone");
  const labelTodo = t("message.taskTodo");
  const labelItem = t("message.taskItem");
  const interactiveTasks = viewing && !readOnly && onToggleTaskItem !== undefined;
  const tagsKey = (tags ?? []).join(" ");
  // Search words are separated by a space: a word is a run of letters and digits, with no space in it.
  const markKey = (markTerms ?? []).join(" ");
  const html = useMemo(
    () =>
      renderMarkdown(markdown, {
        timeSpacer: !failed,
        taskLabels: { done: labelDone, todo: labelTodo, item: labelItem },
        tags: tagsKey === "" ? undefined : tagsKey.split(" "),
        interactiveTasks,
        marks: markKey === "" ? undefined : markKey.split(" "),
      }),
    [markdown, failed, labelDone, labelTodo, labelItem, tagsKey, interactiveTasks, markKey],
  );
  // The message ends with a block (code): the last line has no spacer, the time sits under the block.
  const timeBelow = !failed && !html.includes("message__spacer");

  const plate = useRef<HTMLElement>(null);
  const { state: copyState, copy } = useCopy();
  const [menu, setMenu] = useState<{ position: MenuPosition; selection: string | null } | null>(null);

  // Action row: to the right of the bubble; in a narrow window, slightly above the bubble at its right edge; at the top edge of the feed, inside.
  // Visible while the mouse is in the feed row (and a short delay after it leaves). Details are in useActionsPlacement.
  const { wrap, inside, flip, shown } = useActionsPlacement(`${html}|${mode}`);

  // Bubble width before editing: while the bubble is normal, its width is remembered (and updated on resize); on entering
  // edit mode it is pinned by the `--message-edit-width` variable, so the bubble changes only its height.
  const viewWidth = useRef(0);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode === "editing") return;
    viewWidth.current = element.offsetWidth;
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      viewWidth.current = element.offsetWidth;
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [mode, html]);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode !== "editing") return;
    if (viewWidth.current > 0) element.style.setProperty("--message-edit-width", `${viewWidth.current}px`);
    return () => {
      element.style.removeProperty("--message-edit-width");
    };
  }, [mode]);

  // Editing is over (Enter or Esc): the field disappears, and focus must stay on the entry, otherwise the keyboard loses its place
  // (found in a live check). The bubble is not in the Tab order (tabIndex -1) but takes focus programmatically; Shift+F10 on it
  // opens the menu, Tab goes to its buttons and tags.
  const wasEditing = useRef(false);
  useLayoutEffect(() => {
    if (wasEditing.current && mode !== "editing") plate.current?.focus();
    wasEditing.current = mode === "editing";
  }, [mode]);

  const openMenu = (position: MenuPosition) => {
    if (viewing) setMenu({ position, selection: selectedTextIn(plate.current) });
  };
  const copyNow = (selection: string | null) => void copy(selection ?? markdown);

  // The webview does not follow the link: a mouse click, Enter on a link from the keyboard (the browser turns it into a click) and the middle
  // button (auxclick) are cancelled; the system browser opens the address via `open_external_url` (the scheme is checked in Rust).
  const handleLinkClick = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) return;
    const link = event.target.closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("href");
    // We do not open on the middle button: its click is "cancelled" as a navigation, and opening happens only on the primary one (click).
    if (event.type === "click" && href) onOpenLink?.(href);
  };

  // A `- [ ]` box in the text and a tag: activated by mouse, Enter and Space (they are spans with a role, not real buttons).
  const activate = (target: Element, root: HTMLElement): boolean => {
    const box = target.closest(".task__box");
    if (box && interactiveTasks) {
      const index = [...root.querySelectorAll(".task__box")].indexOf(box);
      onToggleTaskItem?.(index, box.getAttribute("aria-checked") !== "true");
      return true;
    }
    const tag = target.closest(".tag");
    if (tag && onTagClick) {
      onTagClick((tag.textContent ?? "").replace(/^#/, "").toLowerCase());
      return true;
    }
    return false;
  };
  const handleBodyClick = (event: MouseEvent<HTMLDivElement>) => {
    handleLinkClick(event);
    if (event.target instanceof Element) activate(event.target, event.currentTarget);
  };
  const handleBodyKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== " " && event.key !== "Enter") return;
    if (event.target instanceof Element && activate(event.target, event.currentTarget)) event.preventDefault();
  };

  // Menu from the keyboard: Shift+F10 or the menu key on any element of the bubble (row button, checkbox, link) opens it below that element.
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!viewing || !(event.target instanceof Element)) return;
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      openMenu(positionBelow(event.target));
    }
  };

  const toggle = (handler: ((on: boolean) => void) | undefined, current: boolean) =>
    handler ? () => handler(!current) : undefined;
  const checkLocked = !viewing || readOnly || !onToggleDone;

  const classes = ["message"];
  if (highlighted) classes.push("message--highlighted");
  if (timeBelow) classes.push("message--time-below");
  if (isTask) classes.push("message--task");
  if (isTask && taskDone) classes.push("message--task-done");
  if (favorite && mode !== "deleted") classes.push("message--favorite");
  if (byAgent && mode !== "deleted") classes.push("message--agent");
  if (mode === "editing") classes.push("message--editing");
  if (mode === "deleted") classes.push("message--deleted");

  return (
    <>
      <div
        className={[
          "message-wrap",
          showActions || shown || menu ? "message-wrap--actions" : "",
          inside ? "message-wrap--actions-inside" : "",
          inside && flip ? "message-wrap--actions-flip" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        ref={wrap}
        onContextMenu={(event) => {
          if (!viewing) return;
          event.preventDefault();
          openMenu({ x: event.clientX, y: event.clientY });
        }}
        onKeyDown={handleKeyDown}
      >
        <article
          className={classes.join(" ")}
          ref={plate}
          tabIndex={-1}
          onPointerDown={
            onDragStart && viewing && !readOnly && !failed
              ? (event) => {
                  if (plate.current) onDragStart(event, plate.current);
                }
              : undefined
          }
        >
          {isTask ? (
            <button
              type="button"
              role="checkbox"
              className="message__check"
              aria-checked={taskDone}
              aria-disabled={checkLocked || undefined}
              aria-label={t("message.taskCheck")}
              onClick={() => {
                if (!checkLocked) onToggleDone?.(!taskDone);
              }}
            />
          ) : null}
          <div className="message__content">
            {mode === "view" ? quote : null}
            {mode === "editing" ? (
              <MessageEditor initial={markdown} onSave={(text) => onSaveEdit?.(text)} onCancel={() => onCancelEdit?.()} />
            ) : (
              <div
                className="markdown"
                onClick={handleBodyClick}
                onAuxClick={handleLinkClick}
                onKeyDown={handleBodyKeyDown}
                dangerouslySetInnerHTML={{ __html: html }}
              />
            )}
            {mode === "editing" && editError ? (
              <p className="message__error" role="alert">
                {editError}
              </p>
            ) : null}
            {failed ? (
              <p className="message__error">
                {t("message.notSent")}{" "}
                <Button variant="inline" onClick={onRetry} aria-label={t("message.retryLabel")}>
                  {t("message.retry")}
                </Button>
              </p>
            ) : null}
          </div>
          <div className="message__meta">
            {mode === "deleted" ? (
              <span className="message__deleted">
                {t("message.deleted")} ·{" "}
                <Button variant="inline" onClick={onRestore} aria-label={t("message.restoreLabel")}>
                  {t("message.restore")}
                </Button>
              </span>
            ) : (
              <>
                {favorite ? (
                  <span className="message__mark" role="img" aria-label={t("message.favoriteMark")}>
                    <Icon name="favorite" />
                  </span>
                ) : null}
                {byAgent ? <span className="message__origin">{t("message.byAgent")} · </span> : null}
                <time className="message__time" dateTime={sentAt.toISOString()}>
                  {formatTime(sentAt, language, timeZone)}
                </time>
              </>
            )}
          </div>
        </article>
        {viewing ? (
          <MessageActions
            copyState={copyState}
            onCopy={() => copyNow(selectedTextIn(plate.current))}
            isTask={isTask}
            favorite={favorite}
            readOnly={readOnly}
            onEdit={onEdit}
            onToggleTask={toggle(onToggleTask, isTask)}
            onToggleFavorite={toggle(onToggleFavorite, favorite)}
            onMore={openMenu}
            menuOpen={menu !== null}
          />
        ) : null}
      </div>
      {/* The menu is a sibling node, not a child of the wrapper: events from it (right click, keys) do not bubble into the bubble. */}
      {menu && viewing ? (
        <MessageMenu
          position={menu.position}
          isTask={isTask}
          favorite={favorite}
          readOnly={readOnly}
          onEdit={onEdit}
          onToggleTask={toggle(onToggleTask, isTask)}
          onToggleFavorite={toggle(onToggleFavorite, favorite)}
          onChangeDate={onChangeDate}
          onDelete={onDelete}
          moveTargets={moveTargets}
          onMoveTo={onMoveTo}
          onCopy={() => copyNow(menu.selection)}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </>
  );
}
