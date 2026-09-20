import { ArrowRight, Edit2, Mail, Plus, Share2, Star, Trash2 } from "lucide-react";
import { type ReactNode, useEffect } from "react";
import { logger } from "@/lib/logger";
import { Button, IconButton } from "@/ui/components";

// ─── Section wrapper ──────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="bg-m3-surface-container-lowest rounded-[20px] p-8 flex flex-col gap-6 shadow-elevation-1">
      <h2 className="text-xl font-bold font-headline text-m3-on-surface">{title}</h2>
      {children}
    </section>
  );
}

function Label({ children }: { children: ReactNode }) {
  return (
    <span className="text-xs uppercase tracking-widest font-bold text-m3-on-surface-variant">
      {children}
    </span>
  );
}

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-3">
      <Label>{label}</Label>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </div>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export function DesignSystemPage() {
  useEffect(() => {
    logger.info("[DesignSystemPage] mounted");
  }, []);

  return (
    <div className="flex-1 overflow-y-auto bg-m3-surface p-8">
      <div className="max-w-5xl mx-auto flex flex-col gap-8">
        {/* Page header */}
        <header className="mb-4">
          <h1 className="text-4xl font-extrabold font-headline text-m3-primary tracking-tight mb-2">
            Design System
          </h1>
          <p className="text-m3-on-surface-variant text-base">Component showcase — dev only</p>
        </header>

        {/* ── BUTTONS ──────────────────────────────────────────────── */}
        <h2 className="text-2xl font-bold font-headline text-m3-on-surface -mb-4">Buttons</h2>

        {/* Variants */}
        <Section title="Variants">
          <Group label="Primary">
            <Button variant="primary">Confirm Appointment</Button>
            <Button variant="primary" icon={<Plus size={16} />}>
              Create Record
            </Button>
            <Button variant="primary" icon={<ArrowRight size={16} />}>
              View Patient
            </Button>
          </Group>
          <Group label="Secondary">
            <Button variant="secondary">Save Draft</Button>
            <Button variant="secondary" icon={<Mail size={16} />}>
              Message Doctor
            </Button>
          </Group>
          <Group label="Outline">
            <Button variant="outline">Secondary Action</Button>
            <Button variant="outline" icon={<Mail size={16} />}>
              Message Doctor
            </Button>
          </Group>
          <Group label="Ghost">
            <Button variant="ghost">Cancel Changes</Button>
            <Button variant="ghost" icon={<Star size={16} />}>
              View Details
            </Button>
          </Group>
          <Group label="Tonal">
            <Button variant="tonal">Premium Feature</Button>
            <Button variant="tonal" icon={<Star size={16} />}>
              Highlights
            </Button>
          </Group>
          <Group label="Danger">
            <Button variant="danger">Delete Account</Button>
            <Button variant="danger" icon={<Trash2 size={16} />}>
              Discard Draft
            </Button>
          </Group>
        </Section>

        {/* Sizes */}
        <Section title="Sizes">
          <div className="flex flex-wrap items-end gap-8">
            <div className="flex flex-col gap-3">
              <Label>Large</Label>
              <Button variant="tonal" size="lg">
                Hero Action
              </Button>
            </div>
            <div className="flex flex-col gap-3">
              <Label>Default</Label>
              <Button variant="tonal" size="md">
                Standard
              </Button>
            </div>
            <div className="flex flex-col gap-3">
              <Label>Small</Label>
              <Button variant="tonal" size="sm">
                Compact
              </Button>
            </div>
          </div>
          <div className="flex flex-wrap items-end gap-8">
            <div className="flex flex-col gap-3">
              <Label>Large primary</Label>
              <Button variant="primary" size="lg">
                Large
              </Button>
            </div>
            <div className="flex flex-col gap-3">
              <Label>Default primary</Label>
              <Button variant="primary" size="md">
                Default
              </Button>
            </div>
            <div className="flex flex-col gap-3">
              <Label>Small primary</Label>
              <Button variant="primary" size="sm">
                Small
              </Button>
            </div>
          </div>
        </Section>

        {/* Icon Buttons */}
        <Section title="Icon Buttons">
          <Group label="Filled — round (primary)">
            <IconButton
              variant="filled"
              shape="round"
              size="lg"
              icon={<Edit2 size={20} />}
              aria-label="Edit"
            />
            <IconButton
              variant="filled"
              shape="round"
              size="md"
              icon={<Edit2 size={18} />}
              aria-label="Edit"
            />
            <IconButton
              variant="filled"
              shape="round"
              size="sm"
              icon={<Edit2 size={14} />}
              aria-label="Edit"
            />
          </Group>
          <Group label="Outlined — square">
            <IconButton
              variant="outlined"
              shape="square"
              size="lg"
              icon={<Trash2 size={20} />}
              aria-label="Delete"
            />
            <IconButton
              variant="outlined"
              shape="square"
              size="md"
              icon={<Trash2 size={18} />}
              aria-label="Delete"
            />
            <IconButton
              variant="outlined"
              shape="square"
              size="sm"
              icon={<Trash2 size={14} />}
              aria-label="Delete"
            />
          </Group>
          <Group label="Tonal surface — square">
            <IconButton
              variant="tonal"
              shape="square"
              size="lg"
              icon={<Share2 size={20} />}
              aria-label="Share"
            />
            <IconButton
              variant="tonal"
              shape="square"
              size="md"
              icon={<Share2 size={18} />}
              aria-label="Share"
            />
            <IconButton
              variant="tonal"
              shape="square"
              size="sm"
              icon={<Share2 size={14} />}
              aria-label="Share"
            />
          </Group>
          <Group label="Ghost">
            <IconButton
              variant="ghost"
              shape="round"
              size="lg"
              icon={<Star size={20} />}
              aria-label="Favourite"
            />
            <IconButton
              variant="ghost"
              shape="round"
              size="md"
              icon={<Star size={18} />}
              aria-label="Favourite"
            />
            <IconButton
              variant="ghost"
              shape="round"
              size="sm"
              icon={<Star size={14} />}
              aria-label="Favourite"
            />
          </Group>
          <Group label="Danger — destructive actions">
            <IconButton
              variant="danger"
              shape="round"
              size="lg"
              icon={<Trash2 size={20} />}
              aria-label="Delete"
            />
            <IconButton
              variant="danger"
              shape="round"
              size="md"
              icon={<Trash2 size={18} />}
              aria-label="Delete"
            />
            <IconButton
              variant="danger"
              shape="round"
              size="sm"
              icon={<Trash2 size={14} />}
              aria-label="Delete"
            />
          </Group>
        </Section>

        {/* States */}
        <Section title="States">
          <Group label="Normal">
            <Button variant="primary">Normal</Button>
            <Button variant="secondary">Normal</Button>
            <Button variant="outline">Normal</Button>
            <Button variant="tonal">Normal</Button>
          </Group>
          <Group label="Loading">
            <Button variant="primary" loading>
              Saving…
            </Button>
            <Button variant="secondary" loading>
              Loading…
            </Button>
            <Button variant="outline" loading>
              Loading…
            </Button>
          </Group>
          <Group label="Disabled">
            <Button variant="primary" disabled>
              Disabled
            </Button>
            <Button variant="secondary" disabled>
              Disabled
            </Button>
            <Button variant="outline" disabled>
              Disabled
            </Button>
            <Button variant="ghost" disabled>
              Disabled
            </Button>
            <Button variant="tonal" disabled>
              Disabled
            </Button>
            <Button variant="danger" disabled>
              Disabled
            </Button>
          </Group>
          <Group label="Full width">
            <div className="w-full max-w-xs">
              <Button variant="primary" fullWidth>
                Full Width Primary
              </Button>
            </div>
            <div className="w-full max-w-xs">
              <Button variant="outline" fullWidth>
                Full Width Outline
              </Button>
            </div>
          </Group>
        </Section>
      </div>
    </div>
  );
}
