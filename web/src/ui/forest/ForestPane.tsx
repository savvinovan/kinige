import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ForestMode } from "../../api/generated/ForestMode";
import type { ForestYear } from "../../api/generated/ForestYear";
import type { Season } from "../../api/generated/Season";
import { IconButton } from "../controls/IconButton";
import { SegmentedChoice } from "../settings/SegmentedChoice";
import { ForestBoard, type ZoomRequest } from "./ForestBoard";
import { ForestCalendar } from "./ForestCalendar";
import { ForestStrip } from "./ForestStrip";
import "./forest.css";

export interface ForestPaneProps {
  /** `strip` is the collapsed heat map, `expanded` the board or the calendar; hidden is not rendered at all. */
  view: "strip" | "expanded";
  mode: ForestMode;
  /** The table on screen; `null` until the first one arrives. */
  table: ForestYear | null;
  /** The year being shown or loaded (the label while there is no table yet). */
  year: number;
  /** Lower end of "‹ year ›": the year of the oldest entry. */
  minYear: number;
  /** Upper end: the current year. */
  maxYear: number;
  /** The day at the top of the feed. */
  selected: string | null;
  /** The table could not be loaded. */
  failed?: boolean;
  onOpenDay: (date: string) => void;
  onYearChange: (year: number) => void;
  onView: (view: "strip" | "expanded") => void;
  onMode: (mode: ForestMode) => void;
  onHide: () => void;
}

const SEASONS: readonly Season[] = ["winter", "spring", "summer", "autumn"];

/**
 * The forest above the feed: the strip (a flat heat map of the year) or, expanded, the isometric board or the calendar under a
 * header with the year, "Whole year", the season zooms, the "Forest / Calendar" switch, collapse and hide. It is a region of the
 * main window, not a screen of its own; nothing here loads data (the host passes the year table).
 */
export function ForestPane(props: ForestPaneProps) {
  const { t } = useTranslation();
  const { view, mode, table, year, selected, failed = false, onOpenDay } = props;
  const [zoom, setZoom] = useState<ZoomRequest | null>(null);
  const requestZoom = (target: ZoomRequest["target"]) => setZoom((old) => ({ target, nonce: (old?.nonce ?? 0) + 1 }));
  const expanded = view === "expanded";
  const label = String(table?.year ?? year);
  return (
    <section className={expanded ? "forest-pane forest-pane--expanded" : "forest-pane"} aria-label={t("forest.pane")}>
      {expanded ? <ExpandedHeader {...props} label={label} onZoom={requestZoom} /> : <StripHeader {...props} label={label} />}
      {table === null ? (
        failed ? (
          <p className="forest-pane__note">{t("forest.loadFailed")}</p>
        ) : null
      ) : !expanded ? (
        <ForestStrip table={table} selected={selected} onOpenDay={onOpenDay} />
      ) : mode === "calendar" ? (
        <div className="forest-pane__calendar">
          <ForestCalendar table={table} selected={selected} onOpenDay={onOpenDay} />
        </div>
      ) : (
        <ForestBoard table={table} selected={selected} onOpenDay={onOpenDay} zoom={zoom} />
      )}
    </section>
  );
}

interface HeaderProps extends ForestPaneProps {
  label: string;
}

function StripHeader({ label, onView, onHide }: HeaderProps) {
  const { t } = useTranslation();
  return (
    <header className="forest-pane__header">
      <span className="forest-pane__year">{label}</span>
      <div className="forest-pane__spacer" />
      <div className="forest-pane__actions">
        <IconButton small icon="expandPane" label={t("forest.expand")} onClick={() => onView("expanded")} />
        <IconButton small icon="hidePane" label={t("forest.hide")} onClick={onHide} />
      </div>
    </header>
  );
}

function ExpandedHeader({ label, table, minYear, maxYear, year, mode, onYearChange, onMode, onView, onHide, onZoom }: HeaderProps & { onZoom: (target: ZoomRequest["target"]) => void }) {
  const { t } = useTranslation();
  const shown = table?.year ?? year;
  return (
    <header className="forest-pane__header">
      <div className="forest-pane__year-switch" role="group" aria-label={t("forest.year")}>
        <IconButton small icon="chevronLeft" label={t("forest.previousYear")} disabled={shown <= minYear} onClick={() => onYearChange(shown - 1)} />
        <span className="forest-pane__year">{label}</span>
        <IconButton small icon="chevronRight" label={t("forest.nextYear")} disabled={shown >= maxYear} onClick={() => onYearChange(shown + 1)} />
      </div>
      {mode === "forest" ? (
        <>
          <button type="button" className="segmented__option forest-pane__whole" onClick={() => onZoom("year")}>
            {t("forest.wholeYear")}
          </button>
          <div className="segmented" role="group" aria-label={t("forest.zoomTo")}>
            {SEASONS.map((season) => (
              <button type="button" className="segmented__option" key={season} onClick={() => onZoom(season)}>
                {t(`forest.${season}`)}
              </button>
            ))}
          </div>
        </>
      ) : null}
      <div className="forest-pane__spacer" />
      <SegmentedChoice
        label={t("forest.mode")}
        value={mode}
        onChange={onMode}
        options={[
          { value: "forest", label: t("forest.modeForest") },
          { value: "calendar", label: t("forest.modeCalendar") },
        ]}
      />
      <div className="forest-pane__actions">
        <IconButton small icon="collapsePane" label={t("forest.collapse")} onClick={() => onView("strip")} />
        <IconButton small icon="hidePane" label={t("forest.hide")} onClick={onHide} />
      </div>
    </header>
  );
}
