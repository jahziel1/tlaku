import type { help as Source } from '../es/help';

/** English for the `help` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const help: { [K in keyof typeof Source]?: string } = {
  'help.cmd.manual': 'User Manual',
  'help.cmd.news': "What's New",
  'help.cmd.report': 'Report a Problem…',
  'help.cmd.donate': 'Donate to the Project…',
  'help.cmd.about': 'About Tlaku',

  'help.close': 'Close',
  'help.copy': 'Copy',
  'help.copied': 'Copied',
  'help.copyLink': 'Copy the link',
  'help.linkPending': 'Link not published yet',
  'help.linkNote': 'The app does not open links on its own: copy the address and paste it into your browser.',

  'help.manual.title': 'User Manual',
  'help.manual.search': 'Search the manual',
  'help.manual.toc': 'Manual contents',
  'help.manual.none': 'No page mentions “{q}”.',
  'help.manual.page': 'Manual page',
  'help.manual.prev': 'Previous: {title}',
  'help.manual.next': 'Next: {title}',
  'help.manual.missing': 'This page is not in the manual.',
  'help.manual.imageMissing': 'Screenshot not available: {alt}',

  'help.news.title': "What's New",

  'help.about.title': 'About Tlaku',
  'help.about.tagline': 'Free software for laser cutters and engravers.',
  'help.about.version': 'Version',
  'help.about.versionUnknown': 'Not available',
  'help.about.license': 'License',
  'help.about.licenseText':
    'MIT. You may use, copy, modify and distribute Tlaku, including for commercial purposes, as long as you keep the copyright notice. The software is provided without warranty: always review your work before turning on a laser.',
  'help.about.name': 'What the name means',
  'help.about.nameText':
    'Tlaku comes from the Nahuatl word “tlacuilo”: the painter-scribe who drew and wrote the codices. A tlacuilo turned ideas into strokes on a surface, just as this program turns a design into laser movements.',
  'help.about.donate': 'Tlaku is free software and is sustained by donations.',
  'help.about.credits': 'Credits and third-party code',
  'help.about.creditsHint': 'Tlaku uses and adapts open source code under compatible licenses. This is what is recorded in THIRD_PARTY.md:',
  'help.about.honest':
    'Status: the app has not been tested with a real laser machine yet. Everything validated so far was checked against emulators and virtual machines.',

  'help.report.title': 'Report a Problem',
  'help.report.intro':
    'A good report helps us fix the bug faster. Tell us what you were doing, what you expected and what actually happened.',
  'help.report.include': 'If you can, include:',
  'help.report.item1': 'The steps to reproduce the problem, one by one.',
  'help.report.item2': 'Your machine and its controller (for example “Falcon2 22 W, GRBL 1.1”).',
  'help.report.item3': 'The .tlaku file or the design that triggers it, if you can share it.',
  'help.report.item4': 'What the Console tab shows and any warnings from the preview.',
  'help.report.warning':
    'If the problem happened on a real machine, do not repeat the job until you understand it. Try a virtual machine first (Machine, Port: Virtual machine).',
  'help.report.where': 'Where to report it',
  'help.report.info': 'Your installation details',
  'help.report.copyInfo': 'Copy the details',

  'help.donate.title': 'Donate to the Project',
  'help.donate.text':
    'Tlaku is open source (MIT) and has no company behind it: it is sustained by donations. They pay for testing on real machines, installer signing certificates and the time to keep developing it. Donating is voluntary: the program is the same with or without a donation.',
  'help.donate.where': 'Where to donate',
};
