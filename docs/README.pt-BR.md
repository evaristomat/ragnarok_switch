# RagSWITCH Pre-Renewal

[English](../README.md) | **Português brasileiro**

Ragnarok offline para Nintendo Switch, com servidor local, bots e interface para controle. Esta edição tem fontes, saves e manutenção independentes da edição Renewal.

**Estado: 0.1.0-pre12, experimental, em teste manual.** A campanha completa e o desempenho no Switch ainda não foram certificados. O repositório fornece código; os arquivos comerciais do jogo devem ser fornecidos pelo jogador.

## Projeto pessoal e não oficial

**Este é um projeto pessoal, experimental e sem finalidade comercial, voltado a estudo, desenvolvimento e jogo offline individual. Não é uma edição oficial de Ragnarok Online.**

O mantenedor **não é o autor de Ragnarok Online** e não reivindica a criação do jogo, de seu universo, personagens, marcas, músicas ou recursos originais. Esses direitos permanecem com a **Gravity Co., Ltd. e os demais titulares dos respectivos conteúdos**. O código dos projetos utilizados mantém suas próprias licenças, créditos e autoria.

Este projeto **não possui vínculo, patrocínio, aprovação ou endosso da Gravity, de suas licenciadas/distribuidoras ou da Nintendo**. As referências a Ragnarok Online e Nintendo Switch identificam o jogo e a plataforma; não indicam uma parceria. O projeto não opera um serviço oficial, não oferece suporte em nome dessas empresas e não se apresenta como substituto dos serviços oficiais.

A cópia pública não fornece o pacote comercial do jogo, GRFs ou saves pessoais. Os usuários devem obter e utilizar seus próprios recursos conforme as permissões e condições aplicáveis. A disponibilização deste código **não autoriza o uso ou a redistribuição de conteúdo de terceiros**.

Leia o [aviso completo de independência, direitos e ausência de garantias](../docs/DISCLAIMER.md). O caráter pessoal/offline e este aviso não substituem licenças ou autorizações nem garantem imunidade a reivindicações de terceiros.

## Recursos

- Mecânicas pré-Renewal, Base até 99 e primeiras, segundas, transclasses e classes expandidas clássicas. Sem terceiras, quartas ou Doram.
- EXP Base, EXP de classe e drops em 15x; custo de SP das habilidades em zero. Parâmetros em [edition.json](../edition.json).
- Campanha solo **Ecos de Rune-Midgard**, com 33 etapas, objetivos no HUD e orientação de rota. Usa mapas e conteúdo existentes. O Mestre das Classes reconhece a classe atual do personagem.
- Bots com progressão acompanhando o jogador, comerciantes e conversas; configuração de 50 nas cidades, 20 nos campos e teto global de 50.
- HUD própria, câmera no analógico direito e seleção de monstros por ZL/ZR. A mira de habilidades de chão tem confirmação própria.
- Menu de viagens, Mestre das Classes e Asa de Mosca Infinita entregue no primeiro acesso de cada personagem, inclusive personagens existentes após atualização.
- Interface e conteúdo em português brasileiro. A revisão de nomes de NPCs e do Campo dos Aprendizes foi ampliada na pre12; outros diálogos ainda podem exigir revisão.

O protocolo é 20250716 por compatibilidade com os recursos do cliente. Não representa um episódio oficial certificado; alguns mapas usam visuais modernos.

## Começar

| Objetivo | Guia |
| --- | --- |
| Instalar no Switch e preservar saves | [Instalação](../docs/INSTALLATION.md) |
| Compilar a partir dos fontes | [Compilação e toolchain](../docs/BUILDING.md) |
| Fornecer arquivos do jogo e traduções | [Assets e dados locais](../docs/ASSETS.md) |
| Entender cliente, servidor, bots e campanha | [Arquitetura](../docs/ARCHITECTURE.md) |
| Testar e reportar bugs | [Testes](../docs/TESTING.md) e [contribuição](../CONTRIBUTING.md) |
| Conferir licenças e pendências | [Licenças e créditos](../docs/LICENSING.md) |
| Preparar ou publicar um fork | [GitHub e publicação](../docs/PUBLISHING.md) |
| Ver limitações e mudanças recentes | [Estado atual](../docs/STATUS.md) e [CHANGELOG](../CHANGELOG.md) |

## Organização

~~~text
config/       banco inicial em SQL, sem personagens ou dados pessoais
campaign/     definição das etapas da campanha
docs/         guias públicos e referências
scripts/      compilação, verificações e ferramentas (scripts/README.md)
source/       fontes independentes de cliente, servidor e runtime gráfico
vendor/       dependências auxiliares; a exportação pública seleciona o conteúdo
toolchain/    receita dos testes e identificação do ambiente
.github/      verificações leves e modelos de contribuição
~~~

Builds, arquivos do jogo, relatórios e saves ficam em build/, runtime/, outputs/ e evidence/. Essas pastas, as memórias de desenvolvimento e o histórico de criação local não fazem parte da exportação pública.

## Licença e origem

O texto GPLv3 está em [LICENSE](../LICENSE). O projeto agrega componentes com licenças próprias, incluindo GPL, Apache, MIT e OFL: leia [NOTICE](../NOTICE) e [o inventário](../docs/LICENSING.md). Os avisos originais foram preservados. Não há autorização implícita para redistribuir arquivos comerciais do jogo.

Existe uma questão herdada sobre a permissão das tabelas de táticas derivadas de AzzyAI, documentada em [licenças](../docs/LICENSING.md). A organização do repositório não resolve essa permissão; confira esse ponto antes de publicar.
