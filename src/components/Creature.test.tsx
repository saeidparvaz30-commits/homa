import { render } from "@testing-library/react";
import { Creature } from "./Creature";

test("applies state class for idle", () => {
  const { container } = render(<Creature status="idle" name="n" />);
  expect(container.querySelector('[data-state="idle"]')).not.toBeNull();
});
