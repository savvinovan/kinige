import { useTranslation } from "react-i18next";
import type { MirrorStatus } from "../../api/generated/MirrorStatus";
import { StatusNote } from "./StatusNote";

/**
 * Состояние Markdown-копии одной строкой: ошибка (приоритет; записи сохранены, копия догонит), «N дней ждут записи в копию»
 * или «Копия актуальна». Если папка не выбрана и ошибки нет, сказать нечего: строки нет. Ошибка — `role="alert"`.
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
