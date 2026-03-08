import type * as Monaco from 'monaco-editor';

export const oriDarkTheme: Monaco.editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    // Keywords - sage accent
    { token: 'keyword', foreground: '7eb7a6' },
    // Functions - brighter sage
    { token: 'entity.name.function', foreground: 'aad6c4' },
    // Parameters - secondary text
    { token: 'variable.parameter', foreground: 'aca9a3' },
    // Types - copper
    { token: 'type', foreground: 'd9bd7d' },
    // Identifiers - primary text
    { token: 'identifier', foreground: 'd8d7d3' },
    // Strings - warm sand
    { token: 'string', foreground: 'd4976c' },
    { token: 'string.quote', foreground: 'd4976c' },
    { token: 'string.escape', foreground: 'e0b088' },
    { token: 'string.invalid', foreground: 'e84040' },
    // Numbers - soft lavender
    { token: 'number', foreground: 'b4a7d6' },
    { token: 'number.float', foreground: 'b4a7d6' },
    { token: 'number.hex', foreground: 'b4a7d6' },
    // Comments - warm muted
    { token: 'comment', foreground: '908f8b' },
    // Operators - primary text
    { token: 'operator', foreground: 'd8d7d3' },
    // Punctuation - muted
    { token: 'delimiter', foreground: '868380' },
    { token: 'delimiter.bracket', foreground: 'aca9a3' },
    // Constants (true, false, None, etc) - warm like strings
    { token: 'constant', foreground: 'd4976c' },
    // Attributes - between comment and text
    { token: 'annotation', foreground: '908f8b' },
  ],
  colors: {
    // Backgrounds - warm earth tones
    'editor.background': '#161616',
    'editor.foreground': '#d8d7d3',
    'editorLineNumber.foreground': '#908f8b',
    'editorLineNumber.activeForeground': '#aca9a3',
    'editorCursor.foreground': '#7eb7a6',
    // Selection - sage with alpha
    'editor.selectionBackground': '#7eb7a64d',
    'editor.inactiveSelectionBackground': '#7eb7a626',
    'editor.selectionHighlightBackground': '#7eb7a626',
    'editor.selectionHighlightBorder': '#00000000',
    'editor.wordHighlightBackground': '#7eb7a633',
    'editor.wordHighlightBorder': '#00000000',
    'editor.wordHighlightStrongBackground': '#7eb7a64d',
    'editor.wordHighlightStrongBorder': '#00000000',
    'editor.findMatchBackground': '#7eb7a666',
    'editor.findMatchHighlightBackground': '#7eb7a633',
    'editor.findMatchBorder': '#00000000',
    'editor.findMatchHighlightBorder': '#00000000',
    'editor.lineHighlightBackground': '#1a1a1a',
    'editor.lineHighlightBorder': '#242424',
    // Scrollbar - warm
    'scrollbar.shadow': '#00000033',
    'scrollbarSlider.background': '#aca9a333',
    'scrollbarSlider.hoverBackground': '#aca9a359',
    'scrollbarSlider.activeBackground': '#aca9a380',
    // Overview ruler
    'editorOverviewRuler.border': '#00000000',
    'editorOverviewRuler.background': '#161616',
    'editorOverviewRuler.errorForeground': '#908f8b',
    'editorOverviewRuler.warningForeground': '#908f8b',
    'editorOverviewRuler.infoForeground': '#7eb7a6',
    'editorOverviewRuler.selectionHighlightForeground': '#7eb7a64d',
    'editorOverviewRuler.findMatchForeground': '#7eb7a680',
    'editorOverviewRuler.rangeHighlightForeground': '#7eb7a64d',
    'editorOverviewRuler.modifiedForeground': '#908f8b',
    'editorOverviewRuler.addedForeground': '#908f8b',
    'editorOverviewRuler.deletedForeground': '#908f8b',
    'editorOverviewRuler.wordHighlightForeground': '#7eb7a64d',
    'editorOverviewRuler.wordHighlightStrongForeground': '#7eb7a64d',
    'editorOverviewRuler.wordHighlightTextForeground': '#7eb7a64d',
    'editorOverviewRuler.currentContentForeground': '#908f8b',
    'editorOverviewRuler.incomingContentForeground': '#908f8b',
    'editorOverviewRuler.commonContentForeground': '#908f8b',
    // Widget - warm
    'editorWidget.background': '#242424',
    'editorWidget.border': '#333333',
    'editorSuggestWidget.background': '#242424',
    'editorSuggestWidget.border': '#333333',
    'editorSuggestWidget.selectedBackground': '#333333',
    // Misc
    'editorIndentGuide.background': '#242424',
    'editorIndentGuide.activeBackground': '#353535',
    // Bracket matching - sage
    'editorBracketMatch.background': '#7eb7a626',
    'editorBracketMatch.border': '#7eb7a680',
    // Bracket pair colorization - warm muted
    'editorBracketHighlight.foreground1': '#aca9a3',
    'editorBracketHighlight.foreground2': '#aca9a3',
    'editorBracketHighlight.foreground3': '#aca9a3',
    'editorBracketHighlight.foreground4': '#aca9a3',
    'editorBracketHighlight.foreground5': '#aca9a3',
    'editorBracketHighlight.foreground6': '#aca9a3',
    'editorBracketHighlight.unexpectedBracket.foreground': '#aca9a3',
    // Bracket pair guides
    'editorBracketPairGuide.activeBackground1': '#aca9a3',
    'editorBracketPairGuide.activeBackground2': '#aca9a3',
    'editorBracketPairGuide.activeBackground3': '#aca9a3',
    'editorBracketPairGuide.activeBackground4': '#aca9a3',
    'editorBracketPairGuide.activeBackground5': '#aca9a3',
    'editorBracketPairGuide.activeBackground6': '#aca9a3',
    'editorBracketPairGuide.background1': '#00000000',
    'editorBracketPairGuide.background2': '#00000000',
    'editorBracketPairGuide.background3': '#00000000',
    'editorBracketPairGuide.background4': '#00000000',
    'editorBracketPairGuide.background5': '#00000000',
    'editorBracketPairGuide.background6': '#00000000',
    // Overview ruler brackets
    'editorOverviewRuler.bracketMatchForeground': '#7eb7a6',
    // Error squiggles and markers
    'editorError.foreground': '#e84040',
    'editorError.border': '#00000000',
    'editorWarning.foreground': '#db844b',
    'editorWarning.border': '#00000000',
    'editorInfo.foreground': '#7eb7a6',
    'editorInfo.border': '#00000000',
  },
};
