/*--- path: lib/index.ts ---*/
export function f() {}

/*--- path: use.ts ---*/
import { f } from "./lib/index.js";
//       ^ defined: 2
