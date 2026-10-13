// SPDX-License-Identifier: GPL-3.0-or-later
import type { ActionArguments, Binding } from "./types";
export function argumentBindings(args?: ActionArguments): Binding[] {
  return args
    ? [...Object.values(args.fields), ...(args.value ? [args.value] : [])]
    : [];
}
