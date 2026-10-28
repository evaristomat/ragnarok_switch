# Arquitetura e manutenção

## Componentes

| Diretório | Responsabilidade |
| --- | --- |
| source/nostalre-launcher/client | Cliente Rust, renderização, HUD, rede e controle |
| source/nostalre-launcher/server | rAthena modificado, regras clássicas, NPCs, população e SQLite |
| source/nostalre-launcher/client/switch | Entrada Horizon, integração C/C++ e servidor embarcado |
| source/nostalre-launcher/client/switch/romfs/server | Payload de dados/configurações/NPCs incluído no NRO |
| source/bevy-horizon | Runtime Horizon, patches de crates e integração NVK |
| source/mesa-switch | Fontes do driver e compilador de shaders com patches já integrados |
| source/offline-probe | Ferramenta para testes de protocolo local |
| campaign/pre-renewal/campaign.yml | Manifesto da campanha (o arquivo usa conteúdo JSON válido em YAML) |
| source/localization | Catálogo revisado para nomes de NPCs |

As cópias sob source são independentes da edição Renewal. Não há submódulos necessários para puxar o código dessa edição.

## Fontes e payload

A árvore server e a árvore romfs/server não são cópias automaticamente sincronizadas. Alguns scripts geram alterações em ambas, outros preservam um payload específico. Ao alterar um NPC, regra ou configuração, confira o arquivo que efetivamente entra no NRO. Não faça uma sincronização geral que restaure conteúdo Renewal ou traduções antigas.

O gerador scripts/generate_campaign.py lê as 33 etapas, produz arquivos integrados e registra evidências locais. scripts/generate_classic_travel.py mantém destinos do menu. Eles não precisam ser executados em toda compilação; suas saídas atuais estão versionadas.

## Nomes e traduções

O catálogo source/localization/npc-names-ptbr.json alimenta o resultado integrado source/nostalre-launcher/client/lib/game/src/npc_names_ptbr.tsv. A tradução de rótulos é aplicada pelo cliente somente a entidades NPC. Identificadores únicos do servidor, sufixos de referência, jogadores e bots devem permanecer intactos.

## Builds e independência

As bibliotecas dos servidores são recompiladas em modo pré-Renewal. O cliente liga essas bibliotecas e o NVK estaticamente. A extração inicial prepara os arquivos locais e o servidor valida a identidade do banco antes de abrir o perfil. Logs, configuração e saves usam o caminho fixo desta edição no SD.

O ambiente de ferramentas é externo ao clone (Docker/devkitPro/Rust), mas seus comandos de preparação estão em scripts/setup.ps1. Caches, builds, assets privados e evidências não são parte do código público.
