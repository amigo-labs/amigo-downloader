// Action controller shared by every place that can act on a download.
//
// DownloadCard, DownloadCompactRow and DetailPanel each carried their own copy
// of the busy guard, the two-step delete timer and the toast wording, and they
// had already diverged: only the card offered Undo after a delete, and only
// the card reported a specific "delete failed" message.

import {
  pauseDownload,
  resumeDownload,
  retryDownload,
  deleteDownload,
  addDownload,
} from "./api";
import { addToast } from "./toast";
import { crashReport, showFeedbackDialog, ariaAnnouncement, type Download } from "./stores";
import { tr, type Locale } from "./i18n";
import { displayName, canPause, canResume, canRetry } from "./download";

export interface DownloadActions {
  readonly busy: boolean;
  readonly confirmingDelete: boolean;
  pause(): Promise<void>;
  resume(): Promise<void>;
  retry(): Promise<void>;
  /** First call arms the confirmation, second within the window deletes. */
  requestDelete(): Promise<void>;
  /** Delete without the two-step confirmation (context menu, batch). */
  performDelete(): Promise<void>;
  copyUrl(): Promise<void>;
  reportCrash(): void;
  contextMenuItems(): {
    label: string;
    icon: string;
    action: () => void;
    tone?: "default" | "danger";
  }[];
}

/** How long the "Sure?" state stays armed. One value for every call site --
    it used to be 2000ms on rows and 3000ms for batch delete. */
export const CONFIRM_MS = 2500;

export function createDownloadActions(
  get: () => Download,
  getLocale: () => Locale,
): DownloadActions {
  let busy = $state(false);
  let confirmingDelete = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => () => clearTimeout(timer));

  async function run(action: () => Promise<unknown>): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await action();
    } catch {
      addToast("error", tr(getLocale(), "toast.action_failed"), displayName(get()));
    } finally {
      busy = false;
    }
  }

  async function performDelete(): Promise<void> {
    const d = get();
    const url = d.url;
    const label = displayName(d);
    if (busy) return;
    busy = true;
    try {
      await deleteDownload(d.id);
    } catch {
      // Never claim success -- or offer an Undo that would re-enqueue -- when
      // the delete did not commit.
      addToast("error", tr(getLocale(), "toast.delete_failed"), label);
      return;
    } finally {
      busy = false;
    }
    addToast("info", tr(getLocale(), "toast.deleted_one"), label, {
      action: { label: tr(getLocale(), "action.undo"), onAction: () => addDownload(url) },
    });
  }

  return {
    get busy() {
      return busy;
    },
    get confirmingDelete() {
      return confirmingDelete;
    },
    pause: () => run(() => pauseDownload(get().id)),
    resume: () => run(() => resumeDownload(get().id)),
    retry: () => run(() => retryDownload(get().id)),

    async requestDelete() {
      if (!confirmingDelete) {
        confirmingDelete = true;
        // Announced explicitly: the button's own aria-label is static, so a
        // screen reader would otherwise hear nothing change between the two
        // presses and the second press would be a surprise.
        ariaAnnouncement.set(
          `${tr(getLocale(), "action.confirm_delete")}: ${displayName(get())}`,
        );
        timer = setTimeout(() => {
          confirmingDelete = false;
        }, CONFIRM_MS);
        return;
      }
      clearTimeout(timer);
      confirmingDelete = false;
      await performDelete();
    },

    performDelete,

    async copyUrl() {
      try {
        await navigator.clipboard.writeText(get().url);
        addToast("info", tr(getLocale(), "action.copy_url"));
      } catch {
        addToast("error", tr(getLocale(), "action.copy_failed"));
      }
    },

    reportCrash() {
      const d = get();
      crashReport.set({ download_id: d.id, error_message: d.error ?? "" });
      showFeedbackDialog.set(true);
    },

    contextMenuItems() {
      const lang = getLocale();
      const d = get();
      const items: ReturnType<DownloadActions["contextMenuItems"]> = [
        {
          label: tr(lang, "action.copy"),
          icon: "copy",
          action: () => void navigator.clipboard.writeText(d.url),
        },
        {
          label: tr(lang, "action.open_browser"),
          icon: "globe",
          action: () => window.open(d.url, "_blank", "noopener"),
        },
      ];
      if (canPause(d)) {
        items.push({ label: tr(lang, "action.pause"), icon: "pause", action: () => void run(() => pauseDownload(d.id)) });
      } else if (canResume(d)) {
        items.push({ label: tr(lang, "action.resume"), icon: "play", action: () => void run(() => resumeDownload(d.id)) });
      } else if (canRetry(d)) {
        items.push({ label: tr(lang, "action.retry"), icon: "refresh", action: () => void run(() => retryDownload(d.id)) });
      }
      // The menu cannot host a two-step confirm, so it deletes directly --
      // the success toast carries the Undo.
      items.push({
        label: tr(lang, "action.delete"),
        icon: "trash",
        action: () => void performDelete(),
        tone: "danger",
      });
      return items;
    },
  };
}
