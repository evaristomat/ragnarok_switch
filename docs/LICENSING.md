# Licenças, créditos e pendências

O projeto agrega componentes com licenças diferentes. O LICENSE da raiz contém GPLv3 e não substitui licenças permissivas, avisos, atribuições ou condições individuais. Nenhuma licença de código concede direitos sobre arquivos comerciais do jogo.

| Componente | Referência preservada |
| --- | --- |
| nostalRE launcher e integração | source/nostalre-launcher/LICENSE e NOTICE |
| Cliente e componentes derivados | source/nostalre-launcher/client/LICENSE.md, NOTICE e LICENSES/ |
| rAthena e dados derivados | source/nostalre-launcher/server/LICENSE e cabeçalhos dos arquivos |
| bevy-horizon e patches | source/bevy-horizon/LICENSE-MIT e LICENSE-APACHE; avisos de cada patch |
| mesa-switch | source/mesa-switch/docs/license.rst e avisos nos fontes |
| Population engine de origem | vendor/population-engine-origin/third-party/population-engine/LICENSE |
| Noto | avisos OFL e FONT-NOTICES junto às fontes do cliente |

As tabelas geradas do rAthena preservam os termos registrados no NOTICE do cliente. Esse aviso descreve a distribuição dos binários integrados sob GPL-3.0-or-later. Confira os avisos de cada componente ao redistribuir fontes e binários modificados; não substitua tudo por MIT.

## Questão de permissão herdada

O aviso original source/nostalre-launcher/client/docs/third-party-content.md registra que os defaults de lib/ai/src/tactics.rs e configurações de companion_ai.json derivam de AzzyAI e que a permissão para essa adaptação não foi estabelecida no material disponível.

A organização do repositório preserva esse aviso e não modifica o comportamento do jogo para resolver a questão. A exportação local registra publication_approved=false. Antes de torná-la pública, é necessário esclarecer a permissão aplicável ou substituir os defaults por configurações independentes com validação funcional. A revisão atual não representa uma conclusão jurídica sobre cada valor numérico.

## Conteúdo excluído da exportação

GRFs, mapas clássicos obtidos de pacotes externos, pacotes locais de tradução sem licença confirmada, executáveis de terceiros, bancos binários, saves, memórias pessoais e histórico Git privado são omitidos. O arquivo scripts/github/policy.json define a seleção. Um arquivo publicamente acessível em outro repositório não é, por si só, comprovação de permissão de redistribuição.

As fixtures Lua sintéticas e fontes licenciadas são mantidas conforme os avisos originais. Modelos de teste TFLite não utilizados pelo driver NVK são omitidos da cópia pública.

## Créditos

As referências de origem permanecem nos NOTICE dos componentes: nostalRE/nostalro-client e rust-ro (Nicolas Meylan e colaboradores), rAthena Development Team, bevy-horizon, mesa-switch (danfromtico, NaGaa95 e colaboradores), Korangar, Lua, winit, Noto e demais autores identificados nos arquivos. O histórico público novo não apaga autoria nem altera as licenças originais.
