# Assets e dados locais

Os fontes e os arquivos necessários para executar o jogo são distribuídos separadamente. A exportação dos fontes exclui GRFs, mapas de vendor/classic-maps, pacotes locais de tradução, bancos binários e dados pessoais. A release da pre12 fornece separadamente o overlay `ragswitch-assets.grf`; os arquivos base `data.grf` e `official_data.grf` continuam externos ao projeto.

## Recursos esperados

O cliente original usa dados compatíveis com o protocolo 20250716. Para os formatos e organização herdados, veja também source/nostalre-launcher/game-data/README.md. A edição Switch possui configuração própria e espera recursos em sdmc:/switch/ragswitch-pre/game-data.

As referências de obtenção usadas pelo projeto de origem são [WARP0716 / Getting the Client](https://github.com/CrazyBebop/WARP0716#getting-the-client) para o cliente kRO 2025-07-16 e [ROenglishRE / custom officials](https://llchrisll.github.io/ROTPDocs/misc/custom_officials/) para `official_data.grf`. Consulte as instruções e condições dos respectivos projetos. Preserve também as pastas de recursos `System/`, `SystemEN/` e `data/` do cliente compatível. A pasta `BGM/` é opcional e fornece música. Os executáveis Windows e o APK Android não são necessários para executar o NRO no Switch.

O ambiente local atual utiliza data.grf, official_data.grf e um ragswitch-assets.grf produzido localmente, com substituições de mapas e textos para esta edição. **Copiar apenas um data.grf moderno pode causar divergência de terreno com o servidor clássico**, inclusive em Izlude. Não retire as substituições clássicas sem conferir a colisão.

A exportação inclui um manifesto de nomes/hashes dos mapas esperados em config/classic-map-resources.json. Ele identifica arquivos; não fornece autorização para redistribuí-los nem realiza download automático. Um importador público para reconstruir o overlay a partir de dados do usuário ainda está pendente; para instalar a pre12, use o overlay já preparado na release.

**[Baixar `ragswitch-assets.grf` da pre12](https://github.com/evaristomat/ragnarok_switch/releases/download/v0.1.0-pre12/ragswitch-assets.grf).** Copie para `/switch/ragswitch-pre/game-data/ragswitch-assets.grf`. São 21.016.768 bytes e 932 recursos; SHA256 `5fa11aa9056afed4df523264aae86d765a1b6d4b6388ab8a80d79ac964a1619a`, também registrado em `SHA256SUMS.txt` na release. Esse arquivo complementa os dois GRFs base, não os substitui. Não renomeie outro GRF nem remova sua entrada no `config.json`.

## Banco inicial

config/seed.sql contém esquema, dados estáticos necessários e duas contas sintéticas: uma de comunicação local e uma de perfil offline com permissões de GM usadas pelos menus. Não são credenciais de serviços externos. Este projeto é voltado a uma sessão offline local; não exponha os servidores com essas configurações na internet.

~~~powershell
python scripts/setup_seed.py
~~~

O comando cria source/nostalre-launcher/client/switch/romfs/server/ragnarok.db apenas se ele não existir. Não recebe saves como entrada. O conteúdo de personagens permanece vazio; a identidade é ragswitch-prerenewal. O arquivo gerado usa journal_mode=DELETE, exigido pelo SQLite embarcado.

## Traduções

Os nomes de NPCs gerados no cliente e scripts de diálogos revisados estão nos fontes. Os antigos geradores de catálogos locais dependem de pacotes auxiliares e evidências do ambiente do mantenedor, e não são requisitos para compilar os textos já integrados. A exportação não inclui esses pacotes nem promete revisão humana de todos os diálogos.

## Conteúdo preservado

Fontes Noto, avisos OFL e imagens identificadas como arte do projeto nos avisos originais são preservados com suas atribuições. A política não exclui indiscriminadamente todas as imagens ou todos os arquivos Lua: há fixtures sintéticas e recursos licenciados nos projetos de origem. Leia [LICENSING.md](LICENSING.md).
