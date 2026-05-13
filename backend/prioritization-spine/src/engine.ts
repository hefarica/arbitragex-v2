export class BayesianToxicityEngine {
    private priorProbability: number = 0.5; // Neutral start
    private toxicityScore: number = 0.0;

    constructor(initialPrior: number = 0.5) {
        this.priorProbability = initialPrior;
        this.toxicityScore = initialPrior;
    }

    /**
     * Updates the Bayesian probability of route success given new evidence.
     * @param pEvidenceGivenSuccess P(E|Success)
     * @param pEvidenceGivenFailure P(E|Failure)
     * @returns Updated probability of success
     */
    public updateSuccessProbability(pEvidenceGivenSuccess: number, pEvidenceGivenFailure: number): number {
        const pSuccess = this.priorProbability;
        const pFailure = 1 - pSuccess;
        
        const pEvidence = (pEvidenceGivenSuccess * pSuccess) + (pEvidenceGivenFailure * pFailure);
        
        if (pEvidence === 0) return this.priorProbability;
        
        const posterior = (pEvidenceGivenSuccess * pSuccess) / pEvidence;
        this.priorProbability = posterior;
        
        return this.priorProbability;
    }

    /**
     * Evaluates Order Flow Toxicity using basic Bayesian inference.
     * @param flowMetrics Object containing order flow properties like slip/reverts.
     * @returns Toxicity score between 0 and 1.
     */
    public evaluateToxicity(flowMetrics: { revertRate: number; avgSlippageBps: number }): number {
        // Simple Bayesian update for toxicity
        // If revert rate is high, it heavily implies toxic flow.
        const pToxicGivenRevert = 0.9;
        const pToxicGivenSafe = 0.1;
        
        const evidence = flowMetrics.revertRate; // Use revert rate as evidence
        this.toxicityScore = (pToxicGivenRevert * evidence * this.priorProbability) + 
                             (pToxicGivenSafe * (1 - evidence) * (1 - this.priorProbability));
                             
        return this.toxicityScore;
    }
}
