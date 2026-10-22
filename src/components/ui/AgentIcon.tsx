import claude from "@/assets/agents/claude.png";
import codex from "@/assets/agents/codex.png";
import codexDark from "@/assets/agents/codex-dark.png";
import opencode from "@/assets/agents/opencode.png";
import trae from "@/assets/agents/trae.png";
import qoder from "@/assets/agents/qoder.svg";
import workbuddy from "@/assets/agents/workbuddy.svg";
import zcode from "@/assets/agents/zcode.png";
import kimiCode from "@/assets/agents/kimi-code.png";
import minimaxCode from "@/assets/agents/minimax-code.png";
import deepseekHarness from "@/assets/agents/deepseek-harness.svg";
import { normalizeToolId } from "@/lib/tools";

const icons: Partial<Record<string, { src: string; darkSrc?: string }>> = {
  claude: { src: claude },
  codex: { src: codex, darkSrc: codexDark },
  opencode: { src: opencode },
  trae: { src: trae },
  "trae-cn": { src: trae },
  traework: { src: trae },
  qoder: { src: qoder },
  qodercli: { src: qoder },
  workbuddy: { src: workbuddy },
  zcode: { src: zcode },
  "kimi-code": { src: kimiCode },
  "minimax-code": { src: minimaxCode },
  "deepseek-harness": { src: deepseekHarness },
};

export function AgentIcon({ id, name, className = "h-5 w-5" }: { id: string; name: string; className?: string }) {
  const icon = icons[normalizeToolId(id)];
  return <span role="img" aria-label={name} title={name} className={`inline-flex shrink-0 items-center justify-center ${icon ? className : "text-xs font-semibold"}`}>
    {icon ? <>
      <img src={icon.src} alt="" draggable={false} className={`h-full w-full object-contain ${icon.darkSrc ? "dark:hidden" : ""}`} />
      {icon.darkSrc ? <img src={icon.darkSrc} alt="" draggable={false} className="hidden h-full w-full object-contain dark:block" /> : null}
    </> : name}
  </span>;
}
