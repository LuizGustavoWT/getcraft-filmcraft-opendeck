# FilmCraft CLI for OpenDeck

[Português do Brasil](#português-do-brasil) · [Contributing](CONTRIBUTING.md) · [FilmCraft](https://github.com/storytold/filmcraft) · [OpenDeck](https://github.com/nekename/OpenDeck)

An independent Linux-only [OpenDeck](https://github.com/nekename/OpenDeck) plugin for running every available [FilmCraft CLI](https://github.com/storytold/filmcraft) command from a Stream Deck key. It does not bundle FilmCraft: point it at the `filmcraft-cli` installed on your machine.

Each action stores an executable path and a JSON array of arguments. The plugin launches the executable directly, without `sh -c` or another shell. That makes JSON parameters, spaces in paths, and untrusted filenames safe to pass as arguments.

## Features

- A dedicated **FilmCraft Edit Control** action for the application window already open on your desktop—not a separate headless CLI session.
- Key actions for play/pause, add edit (cut), ripple delete, marker navigation, edit-point navigation, shuttle, and undo/redo.
- Encoder modes for frame-accurate jog, variable shuttle (±1× to ±8×), selected-clip audio gain, and selected Lumetri exposure, contrast, or saturation.
- Encoder press is independently assignable (Stop is the default), which makes a D200X dial feel like a compact transport control.
- One configurable **Run FilmCraft CLI** action for keypad keys and encoders.
- Supports every FilmCraft CLI subcommand, option, and future addition: the action forwards the exact JSON argument array you configure.
- Loads the installed CLI's dynamic `commands --json` catalog in the Property Inspector and can create an editable `exec` template for any engine command.
- Configurable executable path, working directory, timeout (1–300 seconds), label, and optional output summary on the key.
- Test a configuration in the Property Inspector before assigning it to a key.
- Linux x86_64 release ZIP created on every CI run; tagged versions are published as GitHub Releases.

## Requirements

- Linux x86_64
- OpenDeck with OpenAction-compatible plugin support
- FilmCraft and `filmcraft-cli` installed and available in `PATH`, or its absolute path entered in the Property Inspector

For **FilmCraft Edit Control**, start the desktop application with its local control port enabled:

```bash
filmcraft --control 9876
```

The edit action only accepts `localhost`, `127.0.0.1`, or `::1`; it cannot contact a remote host. Color dials act on the selected clip's existing Lumetri effect, and audio gain acts on selected audio clips. Each turn is an ordinary FilmCraft undoable operation.

## Install

1. Download `getcraft-filmcraft-opendeck-linux-x86_64.zip` from the [latest release](../../releases/latest).
2. In OpenDeck, open **Plugins** → **Install from file** and select the ZIP. Do not extract or rename the `.sdPlugin` directory.
3. Drag **FilmCraft CLI → Run FilmCraft CLI** to a key.
4. Enter the CLI path if `filmcraft-cli` is not in `PATH`, then enter a JSON argument array.
5. Use **Test command**. A green check means the CLI exited successfully; an alert means that its error summary is available in the Property Inspector and OpenDeck logs.

## Configuration and examples

The **Arguments** field must contain a JSON array of strings. It is not a shell command.

| Goal | Arguments |
| --- | --- |
| List every automation command | `["commands", "--json"]` |
| Start playback through an engine command | `["exec", "timeline.play", "{}"]` |
| Add a marker at 3.5 seconds | `["exec", "timeline.marker.add", "seconds=3.5"]` |
| Import one file into a project | `["--project", "/home/me/edit.fcproj", "import", "/home/me/media/clip.mp4"]` |
| Export a project sequence | `["--project", "/home/me/edit.fcproj", "export", "/home/me/out/final.mp4", "--format", "h264"]` |
| Control a running FilmCraft instance | `["--bridge", "127.0.0.1:7979", "exec", "timeline.play", "{}"]` |

For the last example, start FilmCraft with its control port enabled (for example, `filmcraft --control 7979`). Consult FilmCraft's `filmcraft-cli help`, `filmcraft-cli commands --json`, and `filmcraft-cli describe <id>` for the complete, version-specific command set and parameter schema.

The Property Inspector's **Load command catalog** button runs `filmcraft-cli commands --json` using the current executable and working-directory settings. Selecting an ID creates `["exec", "<id>", "{}"]`; edit the parameters and add any global options you need. Catalog loading is a convenience feature—the generic argument field remains the canonical path to every CLI capability.

## CI/CD and release

Pull requests and pushes to `main` run formatting checks, unit tests, and `scripts/package.sh`. The workflow uploads the Linux installer ZIP as an artifact. Pushing a semantic tag such as `v0.1.0` publishes the same ZIP as a GitHub Release.

To package locally:

```bash
rustup target add x86_64-unknown-linux-gnu
bash scripts/package.sh
```

The output is `dist/getcraft-filmcraft-opendeck-linux-x86_64.zip`, containing exactly `wt.luizgustavo.getcraft.filmcraft.sdPlugin/` at its root.

## Local test installation

On Linux, build the local installation package with:

```bash
bash scripts/install-local.sh
```

By default, the installer only builds the ZIP and does not change OpenDeck's active plugin directory. This prevents development plugins from disrupting an active setup. To deliberately test one plugin in the active OpenDeck installation, opt in explicitly:

```bash
OPENDECK_ALLOW_LIVE_INSTALL=1 bash scripts/install-local.sh
```

Install one development plugin at a time, then use OpenDeck's plugin reload control or restart it manually. The installer itself never restarts or reloads OpenDeck. To target a separate configuration, set `OPENDECK_PLUGINS_DIR` before running the command.

## Platform roadmap

| Platform | Status |
| --- | --- |
| Linux x86_64 | Supported now |
| Windows | Planned; not implemented or packaged yet |
| macOS | Planned; not implemented or packaged yet |

## Security and governance

The plugin never evaluates a shell command. It executes the chosen binary with the strings in the JSON array, and it limits a process to five minutes.

This is an open-contribution repository. Direct write/merge access is reserved for `LuizGustavoWT`; outside contributors use forks and pull requests. GitHub does not have a separate permission that only controls closing issues: users without repository write/triage access cannot close other users' issues, so retaining those permissions only for the maintainer satisfies the governance model. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. FilmCraft and OpenDeck are independent upstream projects with their own licenses and release policies.

---

# Português do Brasil

Plugin independente, somente para Linux, do [OpenDeck](https://github.com/nekename/OpenDeck) para executar todos os comandos disponíveis da [CLI do FilmCraft](https://github.com/storytold/filmcraft) a partir de uma tecla do Stream Deck. O FilmCraft não é incluído no plugin: informe o `filmcraft-cli` instalado na sua máquina.

Cada ação guarda o executável e uma lista JSON de argumentos. O plugin chama o executável diretamente, sem `sh -c` nem outro shell. Assim, JSON, caminhos com espaços e nomes de arquivos não confiáveis são passados com segurança.

## Recursos

- Ação configurável **Run FilmCraft CLI** para teclas e encoders.
- Todos os subcomandos, opções e futuras adições da CLI: a ação repassa exatamente o vetor JSON configurado.
- Catálogo dinâmico de `commands --json` no Property Inspector, com modelo editável de `exec` para qualquer comando interno.
- Caminho do executável, diretório de trabalho, timeout (1–300 s), rótulo e resumo de saída configuráveis.
- Botão de teste no Property Inspector antes de associar a ação a uma tecla.
- ZIP Linux x86_64 gerado em toda execução da CI; tags são publicadas como Releases.

## Requisitos e instalação

Você precisa de Linux x86_64, OpenDeck compatível com OpenAction e FilmCraft com `filmcraft-cli` no `PATH` (ou o caminho absoluto no Property Inspector).

1. Baixe `getcraft-filmcraft-opendeck-linux-x86_64.zip` na [release mais recente](../../releases/latest).
2. No OpenDeck, abra **Plugins** → **Install from file** e escolha o ZIP. Não extraia nem renomeie a pasta `.sdPlugin`.
3. Arraste **FilmCraft CLI → Run FilmCraft CLI** para uma tecla.
4. Configure o caminho da CLI, se necessário, e o vetor JSON de argumentos.
5. Use **Test command**. O check verde indica sucesso; o alerta mostra que houve erro, cujo resumo aparece no inspetor e nos logs do OpenDeck.

## Configuração

O campo **Arguments** precisa ser um vetor JSON de strings; ele não é um comando de shell.

| Objetivo | Arguments |
| --- | --- |
| Listar comandos de automação | `["commands", "--json"]` |
| Iniciar reprodução | `["exec", "timeline.play", "{}"]` |
| Adicionar marcador em 3,5 s | `["exec", "timeline.marker.add", "seconds=3.5"]` |
| Importar arquivo no projeto | `["--project", "/home/me/edit.fcproj", "import", "/home/me/media/clip.mp4"]` |
| Exportar sequência | `["--project", "/home/me/edit.fcproj", "export", "/home/me/out/final.mp4", "--format", "h264"]` |
| Controlar FilmCraft em execução | `["--bridge", "127.0.0.1:7979", "exec", "timeline.play", "{}"]` |

No último exemplo, inicie o FilmCraft com porta de controle, por exemplo `filmcraft --control 7979`. Consulte `filmcraft-cli help`, `filmcraft-cli commands --json` e `filmcraft-cli describe <id>` para todos os comandos e schemas da versão instalada.

**Load command catalog** executa `filmcraft-cli commands --json` com o executável e diretório atuais. Ao escolher um ID, o inspetor cria `["exec", "<id>", "{}"]`; depois edite parâmetros e opções. O catálogo é apenas uma ajuda: o vetor genérico continua cobrindo toda a CLI.

## CI/CD, roadmap e contribuição

PRs e pushes em `main` executam formatação, testes e `scripts/package.sh`. O ZIP vira artefato; uma tag como `v0.1.0` cria uma GitHub Release.

Para gerar o pacote local no Linux, execute `bash scripts/install-local.sh`. Por padrão, o instalador gera o ZIP e não altera o diretório de plugins ativo do OpenDeck, protegendo a configuração em uso. Para testar deliberadamente um único plugin na instalação ativa, confirme explicitamente:

```bash
OPENDECK_ALLOW_LIVE_INSTALL=1 bash scripts/install-local.sh
```

Instale um plugin de desenvolvimento por vez e então recarregue-o pelo OpenDeck ou reinicie manualmente. O instalador jamais reinicia ou recarrega o OpenDeck. Para usar outra configuração, defina `OPENDECK_PLUGINS_DIR` antes do comando.

| Plataforma | Situação |
| --- | --- |
| Linux x86_64 | Suportado agora |
| Windows | Planejado; ainda não implementado nem empacotado |
| macOS | Planejado; ainda não implementado nem empacotado |

Contribuições são bem-vindas via fork e pull request. Apenas `LuizGustavoWT` possui acesso direto para merge e para fechar issues de terceiros. O GitHub não oferece uma permissão separada somente para fechar issues; sem acesso de escrita/triagem, colaboradores públicos não conseguem fechar issues de outras pessoas. Veja [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença

MIT. FilmCraft e OpenDeck são projetos upstream independentes, cada um com sua própria licença.
