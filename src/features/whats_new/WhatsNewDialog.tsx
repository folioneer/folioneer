import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import type { ChangelogSection } from "./parseChangelog";

interface WhatsNewDialogProps {
  version: string;
  sections: ChangelogSection[];
  onDismiss: () => void;
}

type BodyBlock =
  | { kind: "subheading" | "paragraph"; text: string }
  | { kind: "bullets"; items: string[] };

/** Groups plain-text body lines into subheadings, bullet lists, and paragraphs. */
function groupBodyLines(lines: string[]): BodyBlock[] {
  const blocks: BodyBlock[] = [];
  for (const line of lines) {
    if (line.startsWith("### ")) {
      blocks.push({ kind: "subheading", text: line.slice(4) });
      continue;
    }
    if (line.startsWith("- ")) {
      const last = blocks[blocks.length - 1];
      if (last?.kind === "bullets") {
        last.items.push(line.slice(2));
      } else {
        blocks.push({ kind: "bullets", items: [line.slice(2)] });
      }
      continue;
    }
    blocks.push({ kind: "paragraph", text: line });
  }
  return blocks;
}

/**
 * Once-per-upgrade release-notes dialog (WNW-020). Stacks every changelog section
 * released between the last acknowledged version and the current one, newest first
 * (WNW-040). Section content is English-only by design (WNW-060); only the chrome
 * is translated. The single action acknowledges the current version (WNW-050).
 */
export function WhatsNewDialog({ version, sections, onDismiss }: WhatsNewDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      id="whats-new-dialog"
      isOpen
      onClose={onDismiss}
      title={t("whats_new.title", { version })}
      maxWidth="max-w-lg"
      actions={
        <Button id="whats-new-dismiss" onClick={onDismiss}>
          {t("whats_new.dismiss")}
        </Button>
      }
    >
      <div className="flex flex-col gap-5 pb-2">
        {sections.map((section) => (
          <section key={section.version} id={`whats-new-section-${section.version}`}>
            <h4 className="font-medium text-m3-on-surface">
              {section.version}
              {section.date && (
                <span className="ml-2 text-sm font-normal text-m3-on-surface-variant">
                  {section.date}
                </span>
              )}
            </h4>
            {groupBodyLines(section.body).map((block, blockIndex) => {
              const key = `${section.version}-${blockIndex}`;
              if (block.kind === "subheading") {
                return (
                  <h5 key={key} className="mt-2 text-sm font-medium text-m3-on-surface">
                    {block.text}
                  </h5>
                );
              }
              if (block.kind === "bullets") {
                return (
                  <ul key={key} className="mt-1 list-disc pl-5 text-sm leading-relaxed">
                    {block.items.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                );
              }
              return (
                <p key={key} className="mt-1 text-sm leading-relaxed">
                  {block.text}
                </p>
              );
            })}
          </section>
        ))}
      </div>
    </Dialog>
  );
}
