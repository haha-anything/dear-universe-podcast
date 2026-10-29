/** 明暗双主题（任务书 §11.3）：跟随系统 / 强制浅色 / 强制深色。
 * 存 localStorage，启动时先读回来，避免闪一下白屏。 */
export type ThemeMode = "system" | "light" | "dark";

const KEY = "du-theme";

class Theme {
  mode = $state<ThemeMode>("system");
  private mq: MediaQueryList | null = null;
  private onChange = (): void => this.apply();

  init(): () => void {
    const saved = localStorage.getItem(KEY);
    if (saved === "light" || saved === "dark" || saved === "system") {
      this.mode = saved;
    }
    this.mq = window.matchMedia("(prefers-color-scheme: dark)");
    this.apply();
    this.mq.addEventListener("change", this.onChange);
    return () => this.mq?.removeEventListener("change", this.onChange);
  }

  set(mode: ThemeMode): void {
    this.mode = mode;
    localStorage.setItem(KEY, mode);
    this.apply();
  }

  private apply(): void {
    const dark = this.mode === "dark" || (this.mode === "system" && this.mq?.matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }
}

export const theme = new Theme();
