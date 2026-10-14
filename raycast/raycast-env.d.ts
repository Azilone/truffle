/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** truffle binary - Path to the truffle CLI. */
  "trufflePath": string,
  /** Default name filters - Added to name searches that have no in: of their own. Clear it to search the whole disk. */
  "nameFilters": string
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `dig` command */
  export type Dig = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `dig` command */
  export type Dig = {}
}

