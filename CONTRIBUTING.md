# Contribuir

Faça um fork, crie uma branch e envie um pull request com o problema, a alteração e as verificações realizadas. Leia [compilação](docs/BUILDING.md), [estado atual](docs/STATUS.md) e [licenças](docs/LICENSING.md). Use saves descartáveis nos testes.

## Onde alterar

- Cliente e HUD: source/nostalre-launcher/client/.
- Regras, scripts de NPCs e bots: source/nostalre-launcher/server/.
- Conteúdo embarcado: source/nostalre-launcher/client/switch/romfs/server/. Existem arquivos gerados e cópias específicas da edição: confira os dois lados sem sobrescrever o payload indiscriminadamente.
- Campanha: campaign/pre-renewal/campaign.yml e scripts/generate_campaign.py.
- Nomes de NPCs: source/localization/npc-names-ptbr.json e o catálogo indicado em docs/ARCHITECTURE.md.
- Renderer e patches: cliente e source/bevy-horizon/. Preserve a origem dos patches.

Não altere nomes únicos de NPCs para traduzir rótulos: scripts podem usá-los como identificadores. Preserve tokens, variáveis, comandos, cores e placeholders ao traduzir. Grave texto novo em UTF-8 e confira a codificação de arquivos antigos antes de editá-los.

Não envie saves, arquivos GRF, credenciais, logs pessoais ou recursos sem autorização de redistribuição. Descreva como reproduzir usando recursos locais. Mantenha os avisos dos componentes e informe a origem de conteúdo novo.

## Validação

Rode python scripts/github/check_repository.py para conferir a estrutura. Para alterações de jogo, execute os testes relacionados em docs/TESTING.md. Separe resultados no PC e no Switch; teste no PC não comprova fluidez no console.

Um PR de tradução deve incluir texto anterior e revisado e, quando possível, captura sem dados pessoais. Um PR de desempenho deve indicar cenário, quantidade de atores e métricas comparáveis.
