import type * as Monaco from 'monaco-editor';

export const oriDarkTheme: Monaco.editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: false,
  rules: [
    // Functions - accent blue, slightly brighter
    { token: 'entity.name.function', foreground: '6cb6ff' },
    // Parameters - secondary text
    { token: 'variable.parameter', foreground: '9ca0ab' },
    // Keywords - accent blue
    { token: 'keyword', foreground: '569cd6' },
    // Types - success teal
    { token: 'type', foreground: '4ec9b0' },
    // Identifiers - primary text
    { token: 'identifier', foreground: 'e2e4e9' },
    // Strings - warm muted orange
    { token: 'string', foreground: 'd4976c' },
    { token: 'string.quote', foreground: 'd4976c' },
    { token: 'string.escape', foreground: 'ce9178' },
    { token: 'string.invalid', foreground: 'f14c4c' },
    // Numbers - soft purple (complement to teal)
    { token: 'number', foreground: 'b4a7d6' },
    { token: 'number.float', foreground: 'b4a7d6' },
    { token: 'number.hex', foreground: 'b4a7d6' },
    // Comments - muted
    { token: 'comment', foreground: '636874' },
    // Operators - primary text
    { token: 'operator', foreground: 'e2e4e9' },
    // Punctuation - muted
    { token: 'delimiter', foreground: '9ca0ab' },
    { token: 'delimiter.bracket', foreground: '9ca0ab' },
    // Constants (true, false, None, etc)
    { token: 'constant', foreground: 'd4976c' },
    // Attributes
    { token: 'annotation', foreground: '636874' },
  ],
  colors: {
    // Backgrounds - match website tokens
    'editor.background': '#13141a',
    'editor.foreground': '#e2e4e9',
    'editorLineNumber.foreground': '#636874',
    'editorLineNumber.activeForeground': '#9ca0ab',
    'editorCursor.foreground': '#569cd6',
    'editor.selectionBackground': 'rgba(86, 156, 214, 0.3)',
    'editor.inactiveSelectionBackground': 'rgba(86, 156, 214, 0.15)',
    'editor.selectionHighlightBackground': 'rgba(86, 156, 214, 0.15)',
    'editor.selectionHighlightBorder': 'transparent',
    'editor.wordHighlightBackground': 'rgba(86, 156, 214, 0.2)',
    'editor.wordHighlightBorder': 'transparent',
    'editor.wordHighlightStrongBackground': 'rgba(86, 156, 214, 0.3)',
    'editor.wordHighlightStrongBorder': 'transparent',
    'editor.findMatchBackground': 'rgba(86, 156, 214, 0.4)',
    'editor.findMatchHighlightBackground': 'rgba(86, 156, 214, 0.2)',
    'editor.findMatchBorder': 'transparent',
    'editor.findMatchHighlightBorder': 'transparent',
    'editor.lineHighlightBackground': '#1a1b23',
    'editor.lineHighlightBorder': '#2a2b35',
    // Scrollbar
    'scrollbarSlider.background': 'rgba(99, 104, 116, 0.3)',
    'scrollbarSlider.hoverBackground': 'rgba(99, 104, 116, 0.5)',
    'scrollbarSlider.activeBackground': 'rgba(99, 104, 116, 0.7)',
    // Widget
    'editorWidget.background': '#1a1b23',
    'editorWidget.border': '#2a2b35',
    'editorSuggestWidget.background': '#1a1b23',
    'editorSuggestWidget.border': '#2a2b35',
    'editorSuggestWidget.selectedBackground': '#21222c',
    // Misc
    'editorIndentGuide.background': '#2a2b35',
    'editorIndentGuide.activeBackground': '#3c3d4a',
    // Bracket matching - subtle highlight
    'editorBracketMatch.background': 'rgba(86, 156, 214, 0.15)',
    'editorBracketMatch.border': 'rgba(86, 156, 214, 0.5)',
    // Bracket pair colorization - all muted
    'editorBracketHighlight.foreground1': '#9ca0ab',
    'editorBracketHighlight.foreground2': '#9ca0ab',
    'editorBracketHighlight.foreground3': '#9ca0ab',
    'editorBracketHighlight.foreground4': '#9ca0ab',
    'editorBracketHighlight.foreground5': '#9ca0ab',
    'editorBracketHighlight.foreground6': '#9ca0ab',
    'editorBracketHighlight.unexpectedBracket.foreground': '#9ca0ab',
    // Bracket pair guides
    'editorBracketPairGuide.activeBackground1': '#9ca0ab',
    'editorBracketPairGuide.activeBackground2': '#9ca0ab',
    'editorBracketPairGuide.activeBackground3': '#9ca0ab',
    'editorBracketPairGuide.activeBackground4': '#9ca0ab',
    'editorBracketPairGuide.activeBackground5': '#9ca0ab',
    'editorBracketPairGuide.activeBackground6': '#9ca0ab',
    'editorBracketPairGuide.background1': 'transparent',
    'editorBracketPairGuide.background2': 'transparent',
    'editorBracketPairGuide.background3': 'transparent',
    'editorBracketPairGuide.background4': 'transparent',
    'editorBracketPairGuide.background5': 'transparent',
    'editorBracketPairGuide.background6': 'transparent',
    // Overview ruler brackets
    'editorOverviewRuler.bracketMatchForeground': '#569cd6',
    // Error squiggles and markers
    'editorError.foreground': '#f14c4c',
    'editorWarning.foreground': '#cca700',
    'editorInfo.foreground': '#569cd6',
  },
};
