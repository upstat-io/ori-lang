import type * as Monaco from 'monaco-editor';

export const oriDarkTheme: Monaco.editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: 'entity.name.function', foreground: 'DCDCAA' },
    { token: 'variable.parameter', foreground: '9CDCFE' },
    { token: 'keyword', foreground: '569CD6' },
    { token: 'type', foreground: '4EC9B0' },
    { token: 'string', foreground: 'CE9178' },
    { token: 'number', foreground: 'B5CEA8' },
    { token: 'comment', foreground: '6A9955' },
    { token: 'operator', foreground: 'D4D4D4' },
  ],
  colors: {
    'editor.background': '#1e1e1e',
    'editor.foreground': '#d4d4d4',
    'editorLineNumber.foreground': '#858585',
    'editorCursor.foreground': '#aeafad',
    'editor.selectionBackground': '#264f78',
    'editor.lineHighlightBackground': '#2a2a2a',
  },
};
