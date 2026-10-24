/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** Default Width - Default width for text wrapping (leave blank for no wrapping) */
  "defaultWidth": string,
  /** undefined - Whether to copy the result to clipboard by default */
  "defaultCopyToClipboard": boolean
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `text-rewrap-form` command */
  export type TextRewrapForm = ExtensionPreferences & {}
  /** Preferences accessible in the `text-rewrap-clipboard` command */
  export type TextRewrapClipboard = ExtensionPreferences & {}
  /** Preferences accessible in the `text-rewrap-selection` command */
  export type TextRewrapSelection = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `text-rewrap-form` command */
  export type TextRewrapForm = {}
  /** Arguments passed to the `text-rewrap-clipboard` command */
  export type TextRewrapClipboard = {}
  /** Arguments passed to the `text-rewrap-selection` command */
  export type TextRewrapSelection = {}
}

