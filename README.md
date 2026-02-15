# Aion Language v0.5 - The Universal Nexus (Phase 1.5)

Aion est un langage de programmation universel conçu pour l'ère de l'IA. Il combine la performance brute du C++/Inkwell avec la fluidité de Python, tout en intégrant l'IA au cœur de sa syntaxe.

## 🚀 Capacités Actuelles (v0.5)

- **Compilation Native** : Génération de code LLVM IR via Inkwell (Rust).
- **Temporal First-Class** : Support natif des types `Duration` (ex: `5s`) et `Date` (ex: `D2024-01-01`).
- **Arithmétique de Dates** : `Date + Duration` géré nativement par le Type Checker.
- **AI-Native Syntax** : Les blocs `::intent` sont des nœuds de l'AST.
- **Isolation Docker** : Compilateur isolé avec gestion intelligente des signaux et fichiers temporaires uniques.
- **Architecture Modulaire** : Logique de génération de code isolée dans `src/compiler.rs` pour faciliter l'ajout de nouveaux backends.
- **Transpilation SQL** : Support initial pour transformer la logique Aion en SQL (Phase 1.5).

## 🛠 Installation

Le compilateur utilise Docker pour garantir un environnement LLVM 15 stable.

```bash
# Pour compiler un fichier
./aion build hello.ai

# Pour générer la documentation IA
./aion doc hello.ai
```

## 📝 Exemple de Code

```aion
::intent "Calculer la puissance d'un serveur"
struct Server {
    cpu_cores: i64,
    ram_gb: i64
}

fn main() {
    let s = Server { cpu_cores: 64, ram_gb: 128 }
    return s.cpu_cores * s.ram_gb
}
```

## 🏗 Roadmap
- [ ] Backend WebAssembly (WASM) direct.
- [ ] Inférence de type bidirectionnelle avancée.
- [ ] Système d'Acteurs pour la concurrence massive.
