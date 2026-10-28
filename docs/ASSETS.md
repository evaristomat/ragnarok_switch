# Assets e dados locais

Os fontes e os arquivos comerciais necessários para executar o jogo são distribuídos separadamente. A exportação pública exclui GRFs, mapas de vendor/classic-maps, pacotes de tradução sem licença confirmada, bancos binários e dados pessoais.

## Recursos esperados

O cliente original usa dados compatíveis com o protocolo 20250716. Para os formatos e organização herdados, veja também source/nostalre-launcher/game-data/README.md. A edição Switch possui configuração própria e espera recursos em sdmc:/switch/ragswitch-pre/game-data.

O ambiente local atual utiliza data.grf, official_data.grf e um ragswitch-assets.grf produzido localmente, com substituições de mapas e textos para esta edição. **Copiar apenas um data.grf moderno pode causar divergência de terreno com o servidor clássico**, inclusive em Izlude. Não retire as substituições clássicas sem conferir a colisão.

A exportação inclui um manifesto de nomes/hashes dos mapas esperados em config/classic-map-resources.json. Ele identifica arquivos; não fornece autorização para redistribuí-los nem realiza download automático. A preparação pública integral dos overlays PT-BR ainda precisa de um fluxo de importação a partir dos arquivos que cada usuário tem direito de usar. Essa limitação está registrada em STATUS.md.

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
