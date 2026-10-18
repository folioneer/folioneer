import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { WrapperLists } from "@/bindings";
import { AccountForm, type AccountFormData } from "./AccountForm";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, string>) =>
      vars ? `${key}:${Object.values(vars).join(",")}` : key,
    i18n: { language: "en" },
  }),
}));

const lists: WrapperLists = {
  portfolio_country: "FR",
  wrappers: [
    {
      id: "brokerage",
      country: null,
      label_en: "Brokerage",
      label_fr: "Compte-titres",
      label_de: "Depot",
    },
    { id: "fr_pea", country: "FR", label_en: "PEA", label_fr: "PEA", label_de: "PEA" },
    { id: "de_bav", country: "DE", label_en: "bAV", label_fr: "bAV", label_de: "bAV" },
  ],
  countries: ["FR", "DE"],
  offered: [
    { country: "FR", wrapper_ids: ["fr_pea", "brokerage"] },
    { country: "DE", wrapper_ids: ["de_bav", "brokerage"] },
  ],
  generic: ["brokerage"],
};

const formData = (overrides: Partial<AccountFormData> = {}): AccountFormData => ({
  name: "PEA Fortuneo",
  bank_name: "",
  currency: "EUR",
  update_frequency: "ManualMonth",
  management_fees_enabled: false,
  country: "",
  wrapper: "",
  ...overrides,
});

const options = (id: string) =>
  [...(document.getElementById(id) as HTMLSelectElement).options].map((option) => option.value);

describe("AccountForm — country and wrapper (ACC-045, ACC-048)", () => {
  // ACC-045 — every country, searchable: the portfolio's first, named as such; a choice sets
  // the country through setCountry.
  it("offers every country in a searchable list, the portfolio's first", async () => {
    const setCountry = vi.fn();
    render(
      <AccountForm
        formData={formData()}
        handleChange={vi.fn()}
        setCountry={setCountry}
        frequencies={[]}
        wrapperLists={{ ...lists, countries: ["FR", "DE", "ES"] }}
      />,
    );
    const input = screen.getByLabelText("account.form_country_label");
    await userEvent.click(input);
    const names = screen.getAllByRole("option").map((option) => option.textContent);
    expect(names).toEqual(["account.form_country_portfolio:France", "Germany", "Spain"]);
    await userEvent.clear(input);
    await userEvent.type(input, "Spa");
    await userEvent.click(screen.getByRole("option", { name: "Spain" }));
    expect(setCountry).toHaveBeenCalledWith("ES");
  });

  it("offers none, then the country's wrappers and the generic ones", () => {
    const { unmount } = render(
      <AccountForm
        formData={formData()}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        frequencies={[]}
        wrapperLists={lists}
      />,
    );
    expect(options("account-wrapper")).toEqual(["", "fr_pea", "brokerage"]);
    unmount();
    render(
      <AccountForm
        formData={formData({ country: "DE" })}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        frequencies={[]}
        wrapperLists={lists}
      />,
    );
    expect(options("account-wrapper")).toEqual(["", "de_bav", "brokerage"]);
  });

  it("shows neither field until the lists are read", () => {
    render(
      <AccountForm
        formData={formData()}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        frequencies={[]}
        wrapperLists={null}
      />,
    );
    expect(document.getElementById("account-country")).toBeNull();
    expect(document.getElementById("account-wrapper")).toBeNull();
  });
});
