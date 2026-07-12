"""Frozen Test262 module files admitted after static-semantics auditing."""

from pathlib import Path

MODULE_STATIC_SEMANTICS_FILES = frozenset(
    f"language/module-code/{name}"
    for name in (
        "ambiguous-export-bindings/error-export-from-named-as.js",
        "ambiguous-export-bindings/error-export-from-named.js",
        "ambiguous-export-bindings/error-import-named-as.js",
        "ambiguous-export-bindings/error-import-named.js",
        "ambiguous-export-bindings/import-and-export-propagates-binding.js",
        "early-dup-export-id-as.js", "early-dup-export-id.js", "early-export-global.js",
        "early-export-ill-formed-string.js", "early-export-unresolvable.js",
        "early-import-arguments.js", "early-import-as-arguments.js",
        "early-import-as-eval.js", "early-import-eval.js", "eval-export-cls-semi.js",
        "eval-export-fun-semi.js", "export-expname-binding-string.js",
        "export-expname-from-as-unpaired-surrogate.js",
        "export-expname-from-binding-string.js", "export-expname-from-star-string.js",
        "export-expname-from-star-unpaired-surrogate.js", "export-expname-from-star.js",
        "export-expname-from-string-binding.js", "export-expname-from-string-string.js",
        "export-expname-from-string.js", "export-expname-from-unpaired-surrogate.js",
        "export-expname-import-string-binding.js",
        "export-expname-import-unpaired-surrogate.js", "export-expname-string-binding.js",
        "export-expname-unpaired-surrogate.js", "instn-iee-err-not-found-as.js",
        "instn-iee-err-not-found.js", "instn-iee-star-cycle.js",
        "instn-named-err-not-found-as.js", "instn-named-err-not-found.js",
        "instn-resolve-empty-export.js", "instn-resolve-empty-import.js",
        "instn-resolve-err-syntax-1.js", "instn-resolve-err-syntax-2.js",
        "instn-resolve-order-depth.js", "instn-resolve-order-src.js",
        "instn-star-binding.js", "instn-star-equality.js", "instn-star-err-not-found.js",
        "instn-star-id-name.js", "instn-star-iee-cycle.js", "instn-star-star-cycle.js",
        "parse-err-decl-pos-export-arrow-function.js",
        "parse-err-decl-pos-export-block-stmt-list.js",
        "parse-err-decl-pos-export-block-stmt.js",
        "parse-err-decl-pos-export-class-decl-meth-static.js",
        "parse-err-decl-pos-export-class-decl-meth.js",
        "parse-err-decl-pos-export-class-expr-meth-static.js",
        "parse-err-decl-pos-export-class-expr-meth.js",
        "parse-err-decl-pos-export-do-while.js", "parse-err-decl-pos-export-for-const.js",
        "parse-err-decl-pos-export-for-in-const.js",
        "parse-err-decl-pos-export-for-in-let.js",
        "parse-err-decl-pos-export-for-in-lhs.js",
        "parse-err-decl-pos-export-for-in-var.js", "parse-err-decl-pos-export-for-let.js",
        "parse-err-decl-pos-export-for-lhs.js",
        "parse-err-decl-pos-export-for-of-const.js",
        "parse-err-decl-pos-export-for-of-let.js",
        "parse-err-decl-pos-export-for-of-lhs.js",
        "parse-err-decl-pos-export-for-of-var.js", "parse-err-decl-pos-export-for-var.js",
        "parse-err-decl-pos-export-function-decl.js",
        "parse-err-decl-pos-export-function-expr.js",
        "parse-err-decl-pos-export-generator-expr.js",
        "parse-err-decl-pos-export-if-else.js", "parse-err-decl-pos-export-if-if.js",
        "parse-err-decl-pos-export-labeled.js",
        "parse-err-decl-pos-export-object-getter.js",
        "parse-err-decl-pos-export-object-method.js",
        "parse-err-decl-pos-export-object-setter.js",
        "parse-err-decl-pos-export-switch-case-dflt.js",
        "parse-err-decl-pos-export-switch-case.js",
        "parse-err-decl-pos-export-switch-dftl.js",
        "parse-err-decl-pos-export-try-catch-finally.js",
        "parse-err-decl-pos-export-try-catch.js",
        "parse-err-decl-pos-export-try-finally.js",
        "parse-err-decl-pos-export-try-try.js", "parse-err-decl-pos-export-while.js",
        "parse-err-decl-pos-import-arrow-function.js",
        "parse-err-decl-pos-import-block-stmt-list.js",
        "parse-err-decl-pos-import-block-stmt.js",
        "parse-err-decl-pos-import-class-decl-meth-static.js",
        "parse-err-decl-pos-import-class-decl-meth.js",
        "parse-err-decl-pos-import-class-expr-meth-static.js",
        "parse-err-decl-pos-import-class-expr-meth.js",
        "parse-err-decl-pos-import-do-while.js", "parse-err-decl-pos-import-for-const.js",
        "parse-err-decl-pos-import-for-in-const.js",
        "parse-err-decl-pos-import-for-in-let.js",
        "parse-err-decl-pos-import-for-in-lhs.js",
        "parse-err-decl-pos-import-for-in-var.js", "parse-err-decl-pos-import-for-let.js",
        "parse-err-decl-pos-import-for-lhs.js",
        "parse-err-decl-pos-import-for-of-const.js",
        "parse-err-decl-pos-import-for-of-let.js",
        "parse-err-decl-pos-import-for-of-lhs.js",
        "parse-err-decl-pos-import-for-of-var.js", "parse-err-decl-pos-import-for-var.js",
        "parse-err-decl-pos-import-function-decl.js",
        "parse-err-decl-pos-import-function-expr.js",
        "parse-err-decl-pos-import-generator-expr.js",
        "parse-err-decl-pos-import-if-else.js", "parse-err-decl-pos-import-if-if.js",
        "parse-err-decl-pos-import-labeled.js",
        "parse-err-decl-pos-import-object-getter.js",
        "parse-err-decl-pos-import-object-method.js",
        "parse-err-decl-pos-import-object-setter.js",
        "parse-err-decl-pos-import-switch-case-dflt.js",
        "parse-err-decl-pos-import-switch-case.js",
        "parse-err-decl-pos-import-switch-dftl.js",
        "parse-err-decl-pos-import-try-catch-finally.js",
        "parse-err-decl-pos-import-try-catch.js",
        "parse-err-decl-pos-import-try-finally.js",
        "parse-err-decl-pos-import-try-try.js", "parse-err-decl-pos-import-while.js",
        "parse-err-invoke-anon-fun-decl.js", "parse-err-semi-named-export-from.js",
        "parse-err-semi-named-export.js", "parse-export-empty.js",
    )
)

_TLA_SYNTAX_MANIFEST = Path(__file__).with_name("test262_tla_syntax_admission.txt")
MODULE_TLA_SYNTAX_FILES = frozenset(
    line
    for raw_line in _TLA_SYNTAX_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_TLA_RUNTIME_MANIFEST = Path(__file__).with_name("test262_tla_runtime_admission.txt")
MODULE_TLA_RUNTIME_FILES = frozenset(
    line
    for raw_line in _TLA_RUNTIME_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
