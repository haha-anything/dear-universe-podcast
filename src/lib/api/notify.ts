import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

let ready: boolean | null = null;

/** 刷新完成后告知新增集数。首次会请求系统通知权限，用户拒绝则静默跳过。 */
export async function notifyNewEpisodes(count: number): Promise<void> {
  if (count <= 0) return;
  try {
    if (ready === null) {
      ready = await isPermissionGranted();
      if (!ready) ready = (await requestPermission()) === "granted";
    }
    if (!ready) return;
    sendNotification({
      title: "大宇宙播客",
      body: `订阅已更新，新增 ${count} 集`,
    });
  } catch (e) {
    // 通知是加分项，失败不能影响刷新结果
    console.warn("通知发送失败", e);
  }
}
