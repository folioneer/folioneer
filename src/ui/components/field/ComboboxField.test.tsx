import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { ComboboxField } from "./ComboboxField";

interface Item {
  assetId: string;
  assetName: string;
}

const items: Item[] = [
  { assetId: "1", assetName: "Apple Inc" },
  { assetId: "2", assetName: "Banana Corp" },
];

function Harness({
  onChange = vi.fn(),
  initial = "",
}: {
  onChange?: (id: string) => void;
  initial?: string;
}) {
  const [value, setValue] = useState(initial);
  return (
    <ComboboxField
      id="asset"
      label="Asset"
      items={items}
      displayKey="assetName"
      idKey="assetId"
      value={value}
      onChange={(id) => {
        setValue(id);
        onChange(id);
      }}
      searchKeys={["assetName"]}
    />
  );
}

describe("ComboboxField", () => {
  it("shows the selected item's display value", () => {
    render(<Harness initial="2" />);
    expect(screen.getByLabelText("Asset")).toHaveValue("Banana Corp");
  });

  it("opens the full list on click and selects an item", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    // `immediate` opens the dropdown on focus with the full list — no typing needed.
    await user.click(screen.getByLabelText("Asset"));
    await user.click(await screen.findByRole("option", { name: "Apple Inc" }));

    expect(onChange).toHaveBeenCalledWith("1");
  });

  it("offers the create-new option on focus without typing", async () => {
    const user = userEvent.setup();
    const onCreateNew = vi.fn();
    function CreateHarness() {
      const [value, setValue] = useState("");
      return (
        <ComboboxField
          id="asset"
          label="Asset"
          items={items}
          displayKey="assetName"
          idKey="assetId"
          value={value}
          onChange={setValue}
          searchKeys={["assetName"]}
          onCreateNew={onCreateNew}
          createLabel="+ New asset"
        />
      );
    }
    render(<CreateHarness />);

    await user.click(screen.getByLabelText("Asset"));
    await user.click(await screen.findByRole("option", { name: "+ New asset" }));

    expect(onCreateNew).toHaveBeenCalled();
  });
});
