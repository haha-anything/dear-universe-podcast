import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

// 前端崩了必须看得见：release 下没有 devtools，把错误顶到窗口标题上
function surface(message: string): void {
  document.title = `ERR: ${message}`;
}

window.addEventListener("error", (e) => {
  surface(e.message || String(e.error));
});

window.addEventListener("unhandledrejection", (e) => {
  surface(String(e.reason));
});

const target = document.getElementById("app");
if (!target) {
  throw new Error("找不到挂载点 #app");
}

try {
  mount(App, { target });
} catch (e) {
  surface(e instanceof Error ? e.message : String(e));
  throw e;
}
