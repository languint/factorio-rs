import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { rust } from "@codemirror/lang-rust";
import { StreamLanguage } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { oneDark } from "@codemirror/theme-one-dark";

type CodeEditorProps = {
  value: string;
  onChange?: (value: string) => void;
  language: "rust" | "lua";
  readOnly?: boolean;
  className?: string;
};

function languageExtension(language: "rust" | "lua") {
  return language === "rust" ? rust() : StreamLanguage.define(lua);
}

export function CodeEditor({
  value,
  onChange,
  language,
  readOnly = false,
  className,
}: CodeEditorProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && onChangeRef.current) {
        onChangeRef.current(update.state.doc.toString());
      }
    });

    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          languageExtension(language),
          oneDark,
          EditorView.editable.of(!readOnly),
          EditorState.readOnly.of(readOnly),
          updateListener,
          EditorView.theme({
            "&": {
              height: "100%",
              fontSize: "0.875rem",
              lineHeight: "1.35",
            },
            "&.cm-editor": {
              height: "100%",
            },
            ".cm-scroller": {
              overflow: "auto",
              lineHeight: "1.35",
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
            },
            ".cm-content": {
              paddingTop: "0.35rem",
              paddingBottom: "0.35rem",
              lineHeight: "1.35",
            },
            ".cm-line": {
              padding: "0 2px 0 6px",
              lineHeight: "1.35",
            },
            ".cm-gutters": {
              lineHeight: "1.35",
            },
          }),
        ],
      }),
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Mount once per language/readOnly; doc sync is handled separately.
  }, [language, readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) {
      return;
    }
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={hostRef} className={className} />;
}
