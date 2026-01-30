import type * as Monaco from 'monaco-editor';

export const oriDarkTheme: Monaco.editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,
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
    // Selection colors - using 8-char hex (#RRGGBBAA) for alpha
    'editor.selectionBackground': '#569cd64d',
    'editor.inactiveSelectionBackground': '#569cd626',
    'editor.selectionHighlightBackground': '#569cd626',
    'editor.selectionHighlightBorder': '#00000000',
    'editor.wordHighlightBackground': '#569cd633',
    'editor.wordHighlightBorder': '#00000000',
    'editor.wordHighlightStrongBackground': '#569cd64d',
    'editor.wordHighlightStrongBorder': '#00000000',
    'editor.findMatchBackground': '#569cd666',
    'editor.findMatchHighlightBackground': '#569cd633',
    'editor.findMatchBorder': '#00000000',
    'editor.findMatchHighlightBorder': '#00000000',
    'editor.lineHighlightBackground': '#1a1b23',
    'editor.lineHighlightBorder': '#2a2b35',
    // Scrollbar - using hex with alpha
    'scrollbar.shadow': '#00000033',
    'scrollbarSlider.background': '#6368744d',
    'scrollbarSlider.hoverBackground': '#63687480',
    'scrollbarSlider.activeBackground': '#636874b3',
    // Overview ruler (decorations on scrollbar) - all neutral/blue, no red
    'editorOverviewRuler.border': '#00000000',
    'editorOverviewRuler.background': '#13141a',
    'editorOverviewRuler.errorForeground': '#636874',
    'editorOverviewRuler.warningForeground': '#636874',
    'editorOverviewRuler.infoForeground': '#569cd6',
    'editorOverviewRuler.selectionHighlightForeground': '#569cd64d',
    'editorOverviewRuler.findMatchForeground': '#569cd680',
    'editorOverviewRuler.rangeHighlightForeground': '#569cd64d',
    'editorOverviewRuler.modifiedForeground': '#636874',
    'editorOverviewRuler.addedForeground': '#636874',
    'editorOverviewRuler.deletedForeground': '#636874',
    'editorOverviewRuler.wordHighlightForeground': '#569cd64d',
    'editorOverviewRuler.wordHighlightStrongForeground': '#569cd64d',
    'editorOverviewRuler.wordHighlightTextForeground': '#569cd64d',
    'editorOverviewRuler.currentContentForeground': '#636874',
    'editorOverviewRuler.incomingContentForeground': '#636874',
    'editorOverviewRuler.commonContentForeground': '#636874',
    // Widget
    'editorWidget.background': '#1a1b23',
    'editorWidget.border': '#2a2b35',
    'editorSuggestWidget.background': '#1a1b23',
    'editorSuggestWidget.border': '#2a2b35',
    'editorSuggestWidget.selectedBackground': '#21222c',
    // Misc
    'editorIndentGuide.background': '#2a2b35',
    'editorIndentGuide.activeBackground': '#3c3d4a',
    // Bracket matching - subtle highlight (hex with alpha)
    'editorBracketMatch.background': '#569cd626',
    'editorBracketMatch.border': '#569cd680',
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
    'editorBracketPairGuide.background1': '#00000000',
    'editorBracketPairGuide.background2': '#00000000',
    'editorBracketPairGuide.background3': '#00000000',
    'editorBracketPairGuide.background4': '#00000000',
    'editorBracketPairGuide.background5': '#00000000',
    'editorBracketPairGuide.background6': '#00000000',
    // Overview ruler brackets
    'editorOverviewRuler.bracketMatchForeground': '#569cd6',
    // Error squiggles and markers
    'editorError.foreground': '#f14c4c',
    'editorError.border': '#00000000',
    'editorWarning.foreground': '#cca700',
    'editorWarning.border': '#00000000',
    'editorInfo.foreground': '#569cd6',
    'editorInfo.border': '#00000000',
  },
};
