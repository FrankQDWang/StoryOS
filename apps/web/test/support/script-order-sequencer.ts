import { BaseSequencer } from "vitest/node";
import type { TestSpecification } from "vitest/node";

const SCRIPT_FILE_ORDER_VARIABLE = "STORYOS_VITEST_FILE_ORDER";

/**
 * Runs test files in the order that `scripts/verify-project-scope.sh` gives through
 * `STORYOS_VITEST_FILE_ORDER`: project-relative paths separated by `:`.
 *
 * The PostgreSQL HTTP files share one database and one controlled fixture, so the
 * script order is a contract. The Vitest default order depends on the result cache and
 * file size and changes between runs. Without the variable the default order applies.
 */
export class ScriptOrderSequencer extends BaseSequencer {
  override async sort(files: TestSpecification[]): Promise<TestSpecification[]> {
    const order = (process.env[SCRIPT_FILE_ORDER_VARIABLE] ?? "")
      .split(":")
      .filter((entry) => entry.length > 0);
    if (order.length === 0) return super.sort(files);
    const position = (file: TestSpecification): number => {
      const index = order.findIndex((entry) => file.moduleId.endsWith(`/${entry}`));
      if (index < 0) {
        throw new Error(`${file.moduleId} is not listed in ${SCRIPT_FILE_ORDER_VARIABLE}`);
      }
      return index;
    };
    return [...files].sort((left, right) => position(left) - position(right));
  }
}
