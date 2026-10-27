# Motus kawaii (Rust)

Jeu Motus en terminal, Rust + Ratatui. Version Ruby : [motus-kawaii](https://github.com/Gatshub/motus-kawaii), version Go : [motus-go](https://github.com/Gatshub/motus-go).

- 5 à 9 lettres, 6 essais, première lettre donnée
- lettres colorées : rose (bien placé), bleu (présent), gris (absent)
- lettres bien placées reportées verrouillées
- définition Wiktionnaire à la fin (via curl)

## Jouer
```
cargo build --release
./target/release/motus-rust
```
