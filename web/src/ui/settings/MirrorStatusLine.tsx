import { useTranslation } from "react-i18next";
import type { MirrorStatus } from "../../api/generated/MirrorStatus";
import { StatusNote } from "./StatusNote";

/**
 * The Markdown copy state in one line: an error (priority; entries are saved, the copy will catch up), "N дней ждут записи в копию"
 * or "Копия актуальна". If no folder is chosen and there is no error, there is nothing to say: no line. An error is `role="alert"`.
 */
export function MirrorStatusLine({ mirror }: { mirror: MirrorStatus }) {
  const { t } = useTranslation();
  if (mirror.error !== null) {
    return (
      <StatusNote tone="error" icon="alert" role="alert">
        {t(`settings.mirror.error.${mirror.error}`)}
      </StatusNote>
    );
  }
  if (mirror.folder === null) return null;
  if (mirror.pendingDays > 0) {
    return (
      <StatusNote tone="quiet" icon="info" role="status">
        {t("settings.mirror.pending", { count: mirror.pendingDays })}
      </StatusNote>
    );
  }
  return (
    <StatusNote tone="quiet" icon="done" role="status">
      {t("settings.mirror.upToDate")}
    </StatusNote>
  );
}
