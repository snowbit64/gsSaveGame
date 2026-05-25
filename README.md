# gsSaveGame

CLI para **codificar e decodificar** arquivos `.gssg` (GIANTS Software SaveGame Archive).

O formato `.gssg` é usado pelo Farming Simulator para empacotar saves em um único arquivo comprimido.

## Formato do arquivo (engenharia reversa do binário FS26)

```
[4 bytes] Magic marker ("GSSG")
[4 bytes] Tamanho descomprimido (u32 LE)
[N bytes] Payload comprimido com zlib (nível 9)
```

Payload descomprimido:

```
[4 bytes] Número de arquivos (u32 LE)
Para cada arquivo:
  [4 bytes] Tamanho do nome (u32 LE)
  [padded]  Nome do arquivo (alinhado a 4 bytes)
  [4 bytes] Tamanho dos dados (u32 LE)
  [padded]  Dados do arquivo (alinhado a 4 bytes)
```

## Build

```bash
cargo build --release
```

Binário: `./target/release/gsSaveGame`

## Uso

### decoder

Decodifica um arquivo `.gssg` extraindo seus arquivos.

| Flag | Descrição |
|------|-----------|
| `-f, --file <*.gssg>` | Arquivo de entrada |
| `-d, --dir <DIR>` | Diretório de saída |
| `-b, --batch <FILE>` | Múltiplos arquivos .gssg (repetível) |
| `-r, --recursive` | Busca recursiva por .gssg |
| `-h, --help` | Mostra ajuda |

```bash
# Decodificar arquivo
gsSaveGame decoder -f savegame1.gssg -d ./output

# Decodificar múltiplos
gsSaveGame decoder -b save1.gssg -b save2.gssg -d ./output
```

### encoder

Codifica arquivos em um arquivo `.gssg`.

| Flag | Descrição |
|------|-----------|
| `-f, --file <FILE>` | Codifica um arquivo único |
| `-d, --dir <DIR>` | Codifica um diretório |
| `-b, --batch <FILE>` | Adiciona arquivos específicos (repetível) |
| `-r, --recursive` | Recursivo (somente com `--dir`) |
| `-o, --output <OUT.gssg>` | Arquivo de saída |
| `-h, --help` | Mostra ajuda |

```bash
# Codificar diretório recursivo
gsSaveGame encoder -d ./savegame1 -r -o savegame1.gssg

# Codificar arquivo único
gsSaveGame encoder -f careerSavegame.xml -o save.gssg

# Codificar múltiplos arquivos
gsSaveGame encoder -b vehicles.xml -b farmlands.xml -o save.gssg
```

## CI/CD

O workflow compila automaticamente para:
- **Windows** (`x86_64-pc-windows-gnu`)
- **Android ARM64** (`aarch64-linux-android`)

Releases automáticos via tags `v*`.

## Sample

O diretório `sample/` contém um arquivo `.gssg` de exemplo do Farming Simulator 26.
