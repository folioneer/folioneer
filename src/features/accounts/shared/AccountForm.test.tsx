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
  management_fees_enabled: false,
  country: "",
  wrapper: "",
  cost_basis_method: "",
  profile_id: "",
  holder: "",
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
        wrapperLists={null}
      />,
    );
    expect(document.getElementById("account-country")).toBeNull();
    expect(document.getElementById("account-wrapper")).toBeNull();
  });

  // ACC-004 — an account carries no update frequency: the form asks for none
  it("asks for no update frequency", () => {
    render(
      <AccountForm
        formData={formData()}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        wrapperLists={null}
      />,
    );
    expect(document.getElementById("account-update-frequency")).toBeNull();
    expect(screen.queryByText("account.form_frequency_label")).toBeNull();
  });
});

describe("AccountForm — cost basis method (#515, the validated mock)", () => {
  // CBM-014 — left to the country, the method shows the implied one and names its country.
  it("shows the implied method and the country it comes from", () => {
    render(
      <AccountForm
        formData={formData()}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        wrapperLists={lists}
        impliedCostBasis={{ method: "Fifo", country: "DE" }}
      />,
    );
    expect(
      (screen.getByLabelText("account.form_cost_basis_label") as HTMLSelectElement).value,
    ).toBe("Fifo");
    expect(screen.getByText("account.form_cost_basis_implied_fifo:Germany")).toBeInTheDocument();
  });

  // A chosen method hides the hint; a changed one on an edited account warns instead.
  it("warns instead of hinting when an edited account's method changed", () => {
    render(
      <AccountForm
        formData={formData({ cost_basis_method: "WeightedAverage" })}
        handleChange={vi.fn()}
        setCountry={vi.fn()}
        wrapperLists={lists}
        costBasisChanged
        idPrefix="edit-account"
      />,
    );
    expect(document.getElementById("edit-account-cost-basis-warning")).toHaveTextContent(
      "account.form_cost_basis_change_warning",
    );
    expect(screen.queryByText(/form_cost_basis_implied/)).toBeNull();
  });

  // ACC-050 / #767 — typing a holder reaches the form as the holder field; choosing a
  // profile reaches it as the profile field, with the move warning shown when one is given.
  it("passes the holder and the profile on, and shows a move's warning", async () => {
    const handleChange = vi.fn();
    render(
      <AccountForm
        formData={formData({ holder: "", profile_id: "default-profile" })}
        handleChange={handleChange}
        setCountry={vi.fn()}
        wrapperLists={null}
        profiles={[
          { profile: { id: "default-profile", name: null }, account_count: 1, removable: false },
          { profile: { id: "p-1", name: "Parent" }, account_count: 0, removable: true },
        ]}
        profileMoveWarning={{ key: "account.form_profile_move_warning" }}
      />,
    );
    await userEvent.type(screen.getByLabelText("account.form_holder_label"), "C");
    expect(handleChange.mock.calls.at(-1)?.[0].target.name).toBe("holder");
    await userEvent.selectOptions(screen.getByLabelText("account.form_profile_label"), "p-1");
    expect(handleChange.mock.calls.at(-1)?.[0].target.name).toBe("profile_id");
    expect(screen.getByRole("status")).toHaveTextContent("account.form_profile_move_warning");
  });
});
