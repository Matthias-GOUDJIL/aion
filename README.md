# Aion Language v0.1 - The Universal Nexus

Aion est un langage de programmation universel conçu pour l'ère de l'IA. Il combine la performance brute du C++/Rust avec la fluidité de Python, tout en intégrant l'IA au cœur de sa syntaxe.

## 🚀 Capacités Actuelles (v0.1)

- **Compilation Native** : Génération de code LLVM IR ultra-performant.
- **AI-Native Syntax** : Les blocs `::intent` sont des nœuds de l'AST, permettant une compréhension mutuelle Homme-IA.
- **Sécurité Statique** : Type Checker intégré empêchant les opérations illégales (ex: `int + string`).
- **Gestion Mémoire** : Modèle "Smart Ownership" sans Garbage Collector (Zero-Overhead).
- **StdLib Modulaire** :
  - `aion.core` : Manipulation mémoire bas niveau (malloc/memcpy).
  - `aion.std` : Réseau (TCP/HTTP) et Entrées/Sorties.
  - `aion.web` : Manipulation du DOM pour WebAssembly.

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
