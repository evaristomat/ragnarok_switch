# Compilar

## Pré-requisitos

- Git, Python 3.11 ou superior e PowerShell 7 (recomendado no Windows).
- Docker Desktop com containers Linux x86_64, acesso de leitura/escrita à pasta clonada e internet para obter pacotes.
- Espaço para fontes, imagens Docker, caches, NVK e arquivos locais do jogo. O consumo depende dos recursos importados; não use o tamanho do clone como estimativa do ambiente completo.

Clone em uma pasta própria. Os comandos abaixo são executados na raiz do checkout e não dependem da edição Renewal nem de um caminho no C:. A pasta local original fica no E:, mas os comandos novos aceitam outros caminhos de clone.

## Preparar o ambiente

~~~powershell
python scripts/github/check_repository.py
./scripts/setup.ps1 -Stage images
./scripts/setup.ps1 -Stage nvk
./scripts/setup.ps1 -Stage seed
./scripts/setup.ps1 -Stage check
~~~

O estágio images usa o Containerfile preservado em source/bevy-horizon e cria as imagens de compilação, testes e NVK. O estágio nvk compila a cópia já modificada de Mesa incluída nos fontes e gera libvulkan_bh.a. Os intermediários desse procedimento ficam em build/nvk; não há clonagem da edição Renewal. O estágio seed preserva qualquer banco existente e cria um banco novo apenas quando ausente.

**Limite de reprodutibilidade:** a receita fixa o nightly original, mas a imagem-base devkitPro e alguns pacotes continuam móveis. O ambiente originalmente usado está registrado em toolchain/environment.json. A reconstrução completa com essas receitas em uma máquina limpa ainda precisa de validação. Meson, bindgen e cbindgen também são obtidos sem versão congelada na receita NVK. Os identificadores das imagens locais são referência, não URLs de imagens públicas. O link espera GCC 16.1.0: mudança do pacote devkitA64 pode exigir atualizar o caminho do linker em source/bevy-horizon/crates/bevy_horizon/build.rs.

## Compilar

~~~powershell
./scripts/build.ps1 -Stage all
~~~

Também existem os estágios host, servers e client. Após alterar código C/C++ embarcado, compile servers antes de client. O cliente precisa das bibliotecas estáticas dos servidores e do NVK; um clone novo não contém essas bibliotecas compiladas.

- NRO: outputs/switch/ragswitch-pre/ragswitch-pre.nro.
- ELF: outputs/ragswitch-pre.elf.
- Logs: evidence/build-host.log, build-servers.log e build-client.log.
- Caches e intermediários: build/.

Você pode selecionar imagens próprias com o parâmetro -Image dos scripts build.ps1 e test.ps1. Não defina NOSTALRE_CHECK_ONLY em um build distribuível; ele desativa verificações de bibliotecas em compilações voltadas à análise.

## Arquivos para jogar

Compilar o NRO não fornece os arquivos comerciais necessários para jogar. Leia [ASSETS.md](ASSETS.md), prepare o diretório game-data e confira [INSTALLATION.md](INSTALLATION.md). O SQL inicial e o NRO não incluem personagens do desenvolvedor.

## Falhas comuns

- Imagem ausente: execute o estágio images ou informe a imagem local correta.
- libvulkan_bh.a ausente: execute o estágio nvk.
- Banco inicial ausente: execute o estágio seed. Não copie um save da Renewal.
- Bibliotecas nostalre_char/map ausentes: execute build.ps1 -Stage servers.
- Erro de versão de crate: mantenha os lockfiles e patches desta edição. O rust-toolchain.toml do cliente desktop não substitui o nightly usado explicitamente nos scripts Switch.
