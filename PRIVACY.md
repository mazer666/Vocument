# Privacy policy of Vocument

**Last updated:** 2026-10-21
**Applies to:** Vocument for macOS and Vocument for Android, version 0.1.0

Vocument is a document application for lawyers, notaries, translators and
everyone who has to read, mark up and redact documents under a duty of
care. A tool of this kind handles the most private documents a person has.
The privacy policy of Vocument is therefore short, because the application
does not do what most applications do.

---

## 1. The short version

* Vocument **does not collect** personal data.
* Vocument **does not send** anything to a server, because it has no server
  and no network access at all.
* Vocument **does not** ask for an account, and it **does not** contain
  advertising, analytics or telemetry.
* Your documents stay **on your device**. When you share a document, you
  hand it over yourself — through the file dialog, the share sheet or the
  clipboard, the way every other application does it.

## 2. What Vocument stores, and where

| What | Where | Why |
|---|---|---|
| The documents you open | The place you choose, on your device. Vocument does not copy them into a place of its own, with one exception: the recovery store below. | The document belongs to you. |
| The recovery store | A private directory of your user (`~/Library/Application Support/Vocument/vocument_recovery` on macOS, the files directory of the app on Android). It holds a copy of the document as it was when you were working on it, so that a crash does not lose your work. | A crash must not cost you the work. The directory is private to your user (mode 0700, the files 0600) and it is deleted when you close the document without saving. |
| The rail of the profession | The same private directory. It holds the colours and their meanings that you chose for your comments. | Your rail is yours; it should not be lost with a crash. |
| The search index | In the memory of the application, and in the recovery store while the document is open. | The search must work on the device, without a server. |
| The settings of the application | The private directory of your user. | The settings of an application belong to the user. |

## 3. The permissions Vocument asks for, and what it does with them

### macOS

| Permission | Why |
|---|---|
| Files that you choose (`com.apple.security.files.user-selected.read-write`) | Vocument opens the documents that you choose and saves them where you choose. |
| The camera (`com.apple.security.device.camera`) | The scan takes a picture of a document. The picture is processed on the device and is never sent anywhere. |

Vocument does **not** ask for the network (`com.apple.security.network.client`).
The entitlements of the application are in `apps/macos/Resources/VocumentMac.entitlements`.

### Android

| Permission | Why |
|---|---|
| `android.permission.CAMERA` | The scan takes a picture of a document. The picture is processed on the device. |
| The storage that you choose (the system file dialog, Storage Access Framework) | Vocument opens the documents that you choose and saves them where you choose. |

Vocument does **not** ask for `android.permission.INTERNET`. The manifest of
the application is in `apps/android/app/src/main/AndroidManifest.xml`.

## 4. The camera and the pictures

The pictures that the scan takes are processed on the device: the edges are
found, the perspective is corrected, the text is recognised by the OCR that
is part of the application (Tesseract, run locally). No picture is written
into a place of your user that you did not choose, and no picture is sent
anywhere. The application has no way to send it anywhere.

## 5. The children

Vocument is a tool for the professional work of adults. It does not collect
data, and it does not show advertising, so there is nothing that it would
collect from a child either.

## 6. The changes and the inspection

The source of Vocument is open under the GNU Affero General Public License
v3.0 or later. Everyone can read what the application does with your
documents — the source is the proof, not a promise. The source of the
release is named in the "about" area of the application and in the file
`SOURCE.txt` that travels with every released binary.

## 7. The contact

Questions about this policy or about the privacy of a document that Vocument
handles are answered by the maintainers of the project:

* Through the issue tracker of the repository:
  <https://github.com/mazer666/Vocument/issues> — every question and every
  report about the privacy of Vocument belongs there, and the report is
  public.
* **TODO for the project owner:** both stores ask for a contact that a user
  can reach — a postal address and an e-mail address of a person. The name
  and the address have to be entered here before the first upload; a
  placeholder is not enough for the review. The address of the repository
  alone does not answer a request of a user who has no account.

## 8. The changes of this policy

Every change of this policy is a change of the repository, with the date in
the header of this file and in `CHANGELOG.md`.
