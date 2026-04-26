# 🎯 Track: Compiler Hardening & Type Safety
**Phase:** 1.6/1.7 - Rigor & Completeness  
**Status:** 🟡 Proposed  
**Owner:** AI Architect + QA Sentinel  
**Target:** `v0.7.1`

## 📋 Objective
Éliminer les points de fragilité du pipeline de compilation (Lexer → Parser → Checker → LLVM) pour atteindre un état "Production-Ready" : zéro `unwrap()` non justifié, terminaison garantie des blocs IR, et résolution de types déterministe.

## 🔍 Current State Analysis
- `src/compiler.rs` : Utilise massivement `.unwrap()` sur les appels LLVM et les lookups de variables. Certains blocs `if`/`match` peuvent manquer de terminateur explicite.
- `src/checker.rs` : Le `resolve_fuzzy_name` et la substitution de génériques fonctionnent mais manquent de garde-fous contre les collisions de noms ou les types non résolus.
- `src/lib.rs` : Les imports récursifs et le préfixage des déclarations sont fonctionnels mais pourraient générer des conflits si deux modules exportent le même nom.
- Debug statements : Présence de `println!("DEBUG: ...")` et `eprintln!` dans le code de compilation.

## 🛠️ Atomic Tasks

### T1: Error Handling Standardization
- [ ] Remplacer tous les `.unwrap()` dans `compiler.rs` par `.map_err()` ou des retours `Err(String)` explicites.
- [ ] Créer un enum `CompilerError` dans `src/lib.rs` ou `src/compiler.rs` pour typer les erreurs (Lexical, Syntax, Type, LLVM, Runtime).
- [ ] Supprimer les `println!`/`eprintln!` de debug. Remplacer par un système de logs conditionnel (`#[cfg(feature = "debug")]`) ou les déplacer dans un fichier de test dédié.

### T2: LLVM Block Termination Guarantee
- [ ] Implémenter une fonction utilitaire `ensure_block_terminated(builder, default_value)` qui vérifie `get_terminator().is_none()` et injecte un `ret` ou `unreachable` si nécessaire.
- [ ] Appliquer cette fonction à la fin de `compile_function`, `compile_block`, et chaque branche `if`/`match`/`while`.
- [ ] Vérifier la conformité avec l'invariant `GEMINI.md` #4.

### T3: Type Checker Robustness
- [ ] Renforcer `resolve_fuzzy_name` dans `checker.rs` pour éviter les faux positifs (ex: `User` matchant `std.User` par suffixe).
- [ ] Ajouter une validation stricte des génériques : vérifier que tous les paramètres de type sont bien substitués avant la génération LLVM.
- [ ] Garantir que `check_expression` retourne toujours un `Type` concret ou une erreur explicite, jamais `Type::Unknown` en silence.

### T4: Import & Module Isolation
- [ ] Dans `src/lib.rs`, ajouter un namespace automatique basé sur le chemin du fichier importé pour éviter les collisions de symboles globaux.
- [ ] Valider que les déclarations `impl` et `struct` ne sont pas dupliquées lors de l'import récursif.

## ✅ Success Criteria
- `cargo check` passe sans warnings.
- `python3 runner.py` exécute 100% des fixtures sans crash ni fuite mémoire.
- Aucun `unwrap()` résiduel dans `src/compiler.rs` et `src/checker.rs` (sauf dans les tests unitaires).
- Le transpileur SQL et le backend LLVM génèrent un IR valide pour tous les exemples `examples/*.ai`.

## 🔗 Dependencies
- `conductor/tracks/rigor-intelligence/` (alignement phase)
- `conductor/tracks/error-handling/` (standards de reporting)
- `GEMINI.md` (invariants architecturaux v0.7)

## 📅 Next Steps
1. Valider ce plan.
2. Implémenter T1 (Error Handling) → Commit + Test.
3. Implémenter T2 (Block Termination) → Commit + Test.
4. Revue de code & merge.
