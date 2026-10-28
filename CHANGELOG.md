# Mudanças

Este registro resume a integração e não certifica a campanha completa ou o desempenho no console.

## 0.1.0-pre12

- Rótulos de NPCs traduzidos no cliente, preservando identificadores dos scripts e nomes de jogadores/bots.
- Revisão do Campo dos Aprendizes, apresentação de classes e títulos de interlocutores.
- Catálogo local de nomes; revisão de outras quests antigas permanece necessária.

## 0.1.0-pre11

- Carregamento de aparência de atores distribuído entre frames e priorizado por proximidade.
- Atualização de poses de modelos conforme a posição na câmera.
- Setorização de geometria com seleção adaptativa de lotes e controle do preload de efeitos durante movimento.
- Validação gráfica no PC; ganho de FPS no Switch ainda não aprovado.

## 0.1.0-pre10

- Compartilhamento de texturas e atlas para animações de corpo/cabeça.
- Em uma comparação curta no PC, alocações de GPU caíram cerca de 29%; a redução de memória foi muito menor. Não representa redução de 29% na RAM do Switch.

## 0.1.0-pre9

- Comandos do menu deixam de passar pelo bloqueio de mensagens repetidas do chat.
- Opção de retomar o treinamento para personagens com início da campanha preso.

## Integrações anteriores

- Campanha solo com 33 etapas, orientação por HUD/mapa e companheiros temporários.
- Rate 15x e custo de SP das habilidades em zero.
- Ajustes de população, comerciantes, progressão, destinos clássicos e HUD.
- Mestre das Classes pelo menu e Asa de Mosca Infinita.

## Organização para GitHub — 2026-10-28

- Guias públicos, índice de ferramentas, modelos de contribuição e verificação de estrutura.
- Banco inicial em SQL com contas sintéticas, gerado sem sobrescrever bancos existentes.
- Exportação com histórico novo, exclusão de conteúdo local e manifesto de integridade.
- Receitas e pré-requisitos de toolchain documentados; recompilação completa em máquina limpa ainda precisa ser confirmada.
