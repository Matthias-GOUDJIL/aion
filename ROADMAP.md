# Aion Language Roadmap: Towards Self-Hosting (v1.0)

Ce document trace la route technique pour transformer Aion, actuellement un prototype écrit en Rust, en un langage système universel capable de se compiler lui-même.

---

## 🧱 Phase 1 : La Montée en Puissance (v0.4 - v0.6)
*Objectif : Rendre Aion capable d'écrire des programmes complexes et manipulateurs de données.*

### **v0.4 : Le Système de Types Avancé**
- [ ] **Enums & Pattern Matching** : Indispensable pour la logique du compilateur (`match token { ... }`).
- [ ] **Generics** : Introduction de `Type<T>` pour les structures de données réutilisables.
- [ ] **Strings de Première Classe** : Passage du simple pointeur `*u8` à une structure `String` robuste.

### **v0.5 : La Standard Library Étendue**
- [ ] **`std.fs`** : Capacités de lecture/écriture de fichiers.
- [ ] **`std.collections`** : Implémentation des `Vec<T>` et `HashMap<K,V>` directement en Aion.
- [ ] **`std.env`** : Gestion des arguments de la ligne de commande.

### **v0.6 : Le Runtime Natif (Fin du Runtime C)**
- [ ] **Self-Runtime** : Réécriture de `aion_spawn` et du scheduler de threads en Aion via des `syscalls`.
- [ ] **Suppression de GCC/Clang** : Aion ne dépend plus que de LLVM pour le binaire final.

---

## 💎 Phase 1.5 : L'Héritage d'Elo (v0.5 - v0.6)
*Objectif : Intégrer les primitives de données et la sécurité éprouvées par Elo.*

1.  **Gestion Temporelle Native (Time & Duration)**
    *   Primitives de première classe : `Duration` (ex: `5s`, `10ms`), `DateTime`, `Interval`.
    *   Indispensable pour le scheduling et les logs dans Shyrka OS.
2.  **Pipeline Operator (`|>`)**
    *   Syntaxe fluide pour le traitement de données (ex: `data |> filter |> map`).
    *   Améliore la lisibilité des algorithmes d'IA et de transformation.
3.  **Backend SQL & Interopérabilité**
    *   Capacité de compiler des sous-ensembles d'Aion vers du SQL pur.
    *   Permet de projeter la logique métier directement dans la base de données (Zero-Latency).
4.  **Sandboxing de Données (Safe-Mode)**
    *   Mode d'exécution contraint pour les scripts utilisateur (User-Land).
    *   Garantit qu'aucun script ne peut corrompre la mémoire du noyau (inspiré du modèle `_` d'Elo).

---

## 🔄 Phase 2 : Le Grand Portage (v0.7 - v0.9)
*Objectif : Réécrire le cœur du compilateur (actuellement en Rust) dans le langage Aion.*

### **v0.7 : Le Lexer en Aion**
- [ ] Écriture de `lexer.ai`. Validation par le compilateur Rust (Bootstrap Compiler).

### **v0.8 : Le Parser en Aion**
- [ ] Écriture de `parser.ai`. Manipulation de l'AST via les `struct` et `match` d'Aion.

### **v0.9 : Le Backend LLVM en Aion**
- [ ] Génération directe de fichiers texte `.ll` (LLVM IR) à partir de l'AST.

---

## ♾️ Phase 3 : L'Ouroboros (v1.0)
*Objectif : L'auto-hébergement complet (Self-Hosting).*

1. **Compilation Finale** : Utilisation du compilateur Rust pour compiler le code source Aion de `aionc.ai`.
2. **La Boucle** : Utilisation du binaire résultant pour re-compiler son propre code source.
3. **Consécration** : Suppression définitive du code Rust. Aion est 100% autonome.

---

## 📊 État Actuel : v0.3
- [x] Lexer / Parser EBNF v0.1.
- [x] Type Checker (Safety).
- [x] CodeGen LLVM (Native).
- [x] Support des `struct`.
- [x] Concurrence native (Sparks/Spawn).
- [x] Documentation IA automatique.
