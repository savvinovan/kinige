import { useTranslation } from "react-i18next";
import type { Language } from "../../api/generated/Language";
import { CalendarDialog } from "../calendar/CalendarDialog";
import type { SearchUiFilters } from "../search/SearchResults";

interface DateDialogsProps {
  language: Language;
  today: string;
  /** A search "from"/"to" date is being picked. */
  datePick: "from" | "to" | null;
  filters: SearchUiFilters;
  onFilters: (filters: SearchUiFilters) => void;
  onDatePick: (which: "from" | "to" | null) => void;
  /** The entry whose date is being changed, if any. */
  dateTarget: { journalDate: string } | undefined;
  /** Already translated error under the entry's calendar. */
  dateError?: string;
  onPickEntryDate: (date: string) => void;
  onCloseEntryDate: () => void;
}

/** The two calendar dialogs of the main window: the search date filters and "Change date" of an entry. */
export function DateDialogs({
  language,
  today,
  datePick,
  filters,
  onFilters,
  onDatePick,
  dateTarget,
  dateError,
  onPickEntryDate,
  onCloseEntryDate,
}: DateDialogsProps) {
  const { t } = useTranslation();
  return (
    <>
      {datePick ? (
        <CalendarDialog
          value={filters[datePick] ?? today}
          today={today}
          language={language}
          title={t(datePick === "from" ? "search.fromTitle" : "search.toTitle")}
          onPick={(date) => {
            onFilters({ ...filters, [datePick]: date });
            onDatePick(null);
          }}
          onClose={() => onDatePick(null)}
        />
      ) : null}
      {dateTarget ? (
        <CalendarDialog
          value={dateTarget.journalDate}
          today={today}
          language={language}
          error={dateError}
          onPick={onPickEntryDate}
          onClose={onCloseEntryDate}
        />
      ) : null}
    </>
  );
}
